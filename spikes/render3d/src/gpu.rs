//! Offscreen wgpu renderer: picks a software adapter (lavapipe on Linux,
//! WARP on Windows), draws a scene into a multisampled texture with a depth
//! buffer, resolves it and reads the pixels back.

use std::time::{Duration, Instant};

use crate::atlas::{Atlas, MIP_LEVELS};
use crate::math::{Mat4, Vec3};
use crate::mesh::{Meshes, VERTEX_BYTES};

/// Color format of the target. Why not sRGB: the product draws to a non-sRGB
/// surface so that SVG colors reach the screen unchanged (render.rs); the
/// spike does the same, so lighting is computed on sRGB-encoded values.
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Multisampling for smooth edges; 4 is supported by every adapter for
/// these formats (WebGPU guarantees 1 and 4).
pub const SAMPLES: u32 = 4;
const INSTANCE_BYTES: u64 = 64;
const GLOBALS_BYTES: u64 = 96;

/// One microwave to draw: its body color and three model matrices.
pub struct Flyer {
    pub color: usize,
    pub body: Mat4,
    pub door: Mat4,
    pub turntable: Mat4,
}

/// Camera and light of a frame.
pub struct Frame {
    pub view_proj: Mat4,
    /// Unit vector toward the light, in view space.
    pub light: Vec3,
    pub ambient: f32,
    pub diffuse: f32,
}

pub struct Timing {
    /// From encoding to the GPU finishing the frame.
    pub draw: Duration,
    /// Copying the pixels to the CPU.
    pub readback: Duration,
}

pub struct Renderer {
    pub adapter_info: wgpu::AdapterInfo,
    pub anisotropy: u16,
    device: wgpu::Device,
    queue: wgpu::Queue,
    opaque: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    globals: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    meshes_body: [std::ops::Range<u32>; 4],
    mesh_door: std::ops::Range<u32>,
    mesh_turntable: std::ops::Range<u32>,
}

/// Picks a CPU (software) adapter: lavapipe or WARP.
fn pick_adapter(instance: &wgpu::Instance) -> Result<wgpu::Adapter, String> {
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    for a in &adapters {
        let i = a.get_info();
        eprintln!(
            "found adapter: {} ({:?}, {:?})",
            i.name, i.device_type, i.backend
        );
    }
    if let Some(a) = adapters
        .into_iter()
        .find(|a| a.get_info().device_type == wgpu::DeviceType::Cpu)
    {
        return Ok(a);
    }
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: true,
        compatible_surface: None,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("no software adapter: {e}"))
}

fn buffer_with(
    device: &wgpu::Device,
    label: &str,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.len() as u64,
        usage,
        mapped_at_creation: true,
    });
    buffer
        .slice(..)
        .get_mapped_range_mut()
        .expect("mapped at creation")
        .copy_from_slice(bytes);
    buffer.unmap();
    buffer
}

impl Renderer {
    pub fn new(atlas: &Atlas, meshes: &Meshes) -> Result<Self, String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        // Vulkan for lavapipe on Linux, DX12 for WARP on Windows.
        descriptor.backends = wgpu::Backends::VULKAN | wgpu::Backends::DX12;
        // The same shader compiler as the product (SEC-2 in render.rs), so a
        // run on Windows checks the shader with Fxc. The instance is built
        // without with_env(), as in the product.
        descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pick_adapter(&instance)?;
        let adapter_info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("render3d spike"),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .map_err(|e| format!("request_device failed: {e}"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let size = wgpu::Extent3d {
            width: atlas.width,
            height: atlas.height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size,
            mip_level_count: MIP_LEVELS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (level, pixels) in atlas.levels.iter().enumerate() {
            let (w, h) = (atlas.width >> level, atlas.height >> level);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let anisotropy = if adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING)
        {
            8
        } else {
            1
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: anisotropy,
            ..Default::default()
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: GLOBALS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let vertex_attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32];
        let instance_attributes = wgpu::vertex_attr_array![
            4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
        ];
        let buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: VERTEX_BYTES,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attributes,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: INSTANCE_BYTES,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &instance_attributes,
            }),
        ];
        let pipeline =
            |label: &str, entry: &str, depth_write: bool, blend: Option<wgpu::BlendState>| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &buffers,
                    },
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: Some(wgpu::Face::Back),
                        ..Default::default()
                    },
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: DEPTH_FORMAT,
                        depth_write_enabled: Some(depth_write),
                        depth_compare: Some(wgpu::CompareFunction::Less),
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: SAMPLES,
                        ..Default::default()
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: COLOR_FORMAT,
                            blend,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
            };
        let opaque = pipeline("opaque", "fs_opaque", true, None);
        let glass = pipeline(
            "glass",
            "fs_glass",
            false,
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );

        let mut vertex_bytes = Vec::new();
        for v in &meshes.vertices {
            v.write_to(&mut vertex_bytes);
        }
        let index_bytes: Vec<u8> = meshes
            .indices
            .iter()
            .flat_map(|i| i.to_le_bytes())
            .collect();
        let vertices = buffer_with(
            &device,
            "vertices",
            &vertex_bytes,
            wgpu::BufferUsages::VERTEX,
        );
        let indices = buffer_with(&device, "indices", &index_bytes, wgpu::BufferUsages::INDEX);

        Ok(Self {
            adapter_info,
            anisotropy,
            device,
            queue,
            opaque,
            glass,
            bind_group,
            globals,
            vertices,
            indices,
            meshes_body: meshes.body.clone(),
            mesh_door: meshes.door.clone(),
            mesh_turntable: meshes.turntable.clone(),
        })
    }

    /// Draws `flyers` and returns RGBA pixels (rows of `width * 4` bytes).
    pub fn render(
        &self,
        width: u32,
        height: u32,
        frame: &Frame,
        flyers: &[Flyer],
    ) -> (Vec<u8>, Timing) {
        let started = Instant::now();
        let device = &self.device;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let attachment = |label: &str, format, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let msaa = attachment(
            "msaa color",
            COLOR_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let resolved = attachment(
            "resolved color",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = attachment(
            "depth",
            DEPTH_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let msaa_view = msaa.create_view(&Default::default());
        let resolved_view = resolved.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());

        let mut globals = Vec::with_capacity(GLOBALS_BYTES as usize);
        frame.view_proj.write_to(&mut globals);
        for v in [
            frame.light.x,
            frame.light.y,
            frame.light.z,
            0.0,
            frame.ambient,
            frame.diffuse,
            0.0,
            0.0,
        ] {
            globals.extend_from_slice(&f32::to_le_bytes(v));
        }
        self.queue.write_buffer(&self.globals, 0, &globals);

        let mut instance_bytes = Vec::new();
        for f in flyers {
            for m in [&f.body, &f.door, &f.turntable] {
                m.write_to(&mut instance_bytes);
            }
        }
        let instances = buffer_with(
            device,
            "instances",
            &instance_bytes,
            wgpu::BufferUsages::VERTEX,
        );

        // Glass is blended back to front: farthest door first.
        let mut order: Vec<usize> = (0..flyers.len()).collect();
        let depth_of = |i: usize| {
            flyers[i]
                .door
                .transform_point(crate::math::v3(0.0, 0.0, 0.0))
                .z
        };
        order.sort_by(|&a, &b| depth_of(a).total_cmp(&depth_of(b)));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    depth_slice: None,
                    resolve_target: Some(&resolved_view),
                    ops: wgpu::Operations {
                        // Black background (guideline G-5).
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_vertex_buffer(1, instances.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_pipeline(&self.opaque);
            for (i, f) in flyers.iter().enumerate() {
                let base = 3 * i as u32;
                pass.draw_indexed(self.meshes_body[f.color].clone(), 0, base..base + 1);
                pass.draw_indexed(self.mesh_door.clone(), 0, base + 1..base + 2);
                pass.draw_indexed(self.mesh_turntable.clone(), 0, base + 2..base + 3);
            }
            pass.set_pipeline(&self.glass);
            for &i in &order {
                let base = 3 * i as u32;
                pass.draw_indexed(self.mesh_door.clone(), 0, base + 1..base + 2);
            }
        }
        let row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &resolved,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            size,
        );
        self.queue.submit([encoder.finish()]);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device is lost");
        let draw = started.elapsed();

        let started = Instant::now();
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("mapping failed"));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device is lost");
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        {
            let data = slice.get_mapped_range().expect("the buffer is mapped");
            for y in 0..height {
                let start = (y * row) as usize;
                pixels.extend_from_slice(&data[start..start + (width * 4) as usize]);
            }
        }
        readback.unmap();
        (
            pixels,
            Timing {
                draw,
                readback: started.elapsed(),
            },
        )
    }
}
