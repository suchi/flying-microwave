//! wgpu setup and the 3D renderer (ADR-0026).
//!
//! Each frame clears the surface to the background color and the depth
//! buffer, draws every body and turntable (opaque images, no alpha test)
//! and the opaque texels of every door with depth, and then the translucent
//! door glass back to front without writing depth ([`crate::model`],
//! `mesh.wgsl`). The face images are
//! rasterized from the SVG assets into one atlas with mipmaps when the first
//! surface is set up; the atlas and the meshes are shared by every window
//! (plan 0005, D-13). Nothing is rasterized per frame.
//!
//! No multisampling (plan 0005, D-13): the edges are judged with
//! `v0.3.0-alpha.2`. Anisotropic filtering is 8 where the adapter supports it.
//!
//! Hardening (docs/design/architecture.md):
//! - SEC-2: the DX12 shader compiler is set to `Fxc` explicitly, and the
//!   backend is DX12 only.
//! - The instance is built without `with_env()`, so `WGPU_*` environment
//!   variables cannot change the backend or the compiler.
//! - Power preference is LowPower (do not wake a discrete GPU needlessly).

use std::cell::OnceCell;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use flying_microwave_core::flight::{self, FlyerInstance};
use flying_microwave_core::sim::BODY_COLORS;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::atlas;
use crate::diag;
use crate::gpu_limit;
use crate::log::log;
use crate::model;

/// Adapter, device and queue shared by every window, and the scene that
/// every window draws.
pub struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Made with the first surface; `None` inside when it could not be made.
    scene: OnceCell<Option<Scene>>,
}

/// Creates the wgpu instance with the explicit backend and shader compiler.
pub fn create_instance() -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::DX12;
    descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
    wgpu::Instance::new(descriptor)
}

/// Creates a surface for a window. The surface keeps the window alive.
pub fn create_surface(
    instance: &wgpu::Instance,
    window: &Arc<Window>,
) -> Result<wgpu::Surface<'static>, String> {
    instance
        .create_surface(Arc::clone(window))
        .map_err(|e| format!("create_surface failed: {e}"))
}

/// Requests the adapter and the device. Blocks until both are ready.
pub fn request_gpu(instance: &wgpu::Instance, surface: &wgpu::Surface<'_>) -> Result<Gpu, String> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: Some(surface),
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("request_adapter failed: {e}"))?;
    let info = adapter.get_info();
    log!(
        "adapter: {} ({:?}, {:?}, driver {})",
        info.name,
        info.device_type,
        info.backend,
        info.driver
    );
    // Why these progress lines: on a runner without a GPU (WARP) setup did not
    // finish within the watchdog's limit, and the log could not tell which
    // call was waiting (plan 0003, smoke test of PR #41).
    log!("requesting the device");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("flying-microwave device"),
        // Why: ask for what the adapter offers, so large (4K) surfaces are
        // never rejected by conservative default limits.
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| format!("request_device failed: {e}"))?;
    log!("device ready");
    // By default wgpu panics on uncaptured errors; log them instead.
    device.on_uncaptured_error(Arc::new(|error| log!("wgpu error: {error}")));
    Ok(Gpu {
        adapter,
        device,
        queue,
        scene: OnceCell::new(),
    })
}

/// Turns a panic inside wgpu into an `Err`, so that the caller can keep the
/// black windows (W-9) instead of crashing.
pub fn guarded<T>(what: &str, init: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(init)) {
        Ok(result) => result,
        Err(_) => Err(format!("{what} panicked")),
    }
}

/// Runs `create` and turns the wgpu errors it raises into an `Err`.
///
/// Why: wgpu reports a failed resource creation (a shader the compiler
/// rejects, too little memory) through the error handler and hands back an
/// invalid object, which would then fail on every frame.
fn checked<T>(
    device: &wgpu::Device,
    what: &str,
    create: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let out_of_memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let validation = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let result = create();
    // Scopes are popped in the reverse order of pushing.
    let errors = [internal.pop(), validation.pop(), out_of_memory.pop()]
        .map(pollster::block_on)
        .into_iter()
        .flatten()
        .map(|e| e.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        result
    } else {
        Err(format!("{what} failed: {}", errors.join("; ")))
    }
}

/// Depth buffer format.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Bytes of the per-surface uniforms: the projection, the light and its strengths.
const GLOBALS_BYTES: u64 = 96;
/// Anisotropic filtering where the adapter supports it (plan 0005, D-13).
const ANISOTROPY: u16 = 8;
/// Instance buffer capacity at start, in flyers.
const INITIAL_FLYERS: u64 = 32;

/// What every window draws with: the face atlas, the meshes and the shader.
struct Scene {
    shader: wgpu::ShaderModule,
    globals_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    atlas: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    body: [Range<u32>; BODY_COLORS as usize],
    door: Range<u32>,
    turntable: Range<u32>,
}

impl Gpu {
    /// The shared scene, made on the first call. `None` when it could not
    /// be made (logged once): then only the background is drawn.
    fn scene(&self) -> Option<&Scene> {
        self.scene
            .get_or_init(|| match Scene::new(self) {
                Ok(scene) => Some(scene),
                Err(e) => {
                    log!("{e}: drawing the background only");
                    None
                }
            })
            .as_ref()
    }
}

/// Makes a buffer and queues writing `bytes` into it.
fn buffer_with(gpu: &Gpu, label: &str, bytes: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.len() as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue.write_buffer(&buffer, 0, bytes);
    buffer
}

impl Scene {
    fn new(gpu: &Gpu) -> Result<Self, String> {
        let started = Instant::now();
        let images = atlas::rasterize_faces()?;
        let rasterized = started.elapsed();
        let atlas = atlas::build(&images)?;
        drop(images);
        let max = gpu.device.limits().max_texture_dimension_2d;
        if atlas.width > max || atlas.height > max {
            return Err(format!(
                "the face atlas of {}x{} exceeds the device limit of {max}",
                atlas.width, atlas.height
            ));
        }
        let meshes = model::build(&atlas.uvs);
        // Why the downlevel flag: wgpu ignores the clamp where the adapter
        // cannot filter anisotropically, so report what is actually used.
        let anisotropy = if gpu
            .adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING)
        {
            ANISOTROPY
        } else {
            1
        };
        let device = &gpu.device;
        let scene = checked(device, "creating the face atlas and the meshes", || {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mesh shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("mesh.wgsl").into()),
            });
            let size = wgpu::Extent3d {
                width: atlas.width,
                height: atlas.height,
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("face atlas"),
                size,
                mip_level_count: atlas::MIP_LEVELS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Why not sRGB: the surface is not sRGB either, so the
                // colors of the SVG reach the screen unchanged.
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            for (level, pixels) in atlas.levels.iter().enumerate() {
                let (w, h) = (atlas.width >> level, atlas.height >> level);
                gpu.queue.write_texture(
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
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("face atlas"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                anisotropy_clamp: anisotropy,
                ..Default::default()
            });
            let globals_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("mesh globals"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(GLOBALS_BYTES),
                        },
                        count: None,
                    }],
                });
            let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("face atlas"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let atlas_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("face atlas"),
                layout: &atlas_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mesh"),
                bind_group_layouts: &[Some(&globals_layout), Some(&atlas_layout)],
                immediate_size: 0,
            });
            let mut vertex_bytes = Vec::new();
            for v in &meshes.vertices {
                v.write_to(&mut vertex_bytes);
            }
            let index_bytes: Vec<u8> = meshes
                .indices
                .iter()
                .flat_map(|i| i.to_ne_bytes())
                .collect();
            let vertices = buffer_with(
                gpu,
                "mesh vertices",
                &vertex_bytes,
                wgpu::BufferUsages::VERTEX,
            );
            let indices = buffer_with(gpu, "mesh indices", &index_bytes, wgpu::BufferUsages::INDEX);
            Ok(Self {
                shader,
                globals_layout,
                pipeline_layout,
                atlas: atlas_group,
                vertices,
                indices,
                body: meshes.body.clone(),
                door: meshes.door.clone(),
                turntable: meshes.turntable.clone(),
            })
        })?;
        log!(
            "face atlas: {}x{}, {} levels, anisotropy {anisotropy}, rasterized in {} ms, ready in {} ms",
            atlas.width,
            atlas.height,
            atlas::MIP_LEVELS,
            rasterized.as_millis(),
            started.elapsed().as_millis()
        );
        Ok(scene)
    }
}

/// What one surface draws the scene with: the pipelines for its format, its
/// depth buffer, its uniforms and its instance buffer.
struct Meshes {
    /// Body and turntable: opaque images, no alpha test.
    solid: wgpu::RenderPipeline,
    /// Door: the opaque texels.
    cutout: wgpu::RenderPipeline,
    glass: wgpu::RenderPipeline,
    depth: wgpu::TextureView,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    /// Instance bytes of the current frame, kept to reuse the allocation.
    bytes: Vec<u8>,
}

impl Meshes {
    fn new(gpu: &Gpu, scene: &Scene, config: &wgpu::SurfaceConfiguration) -> Result<Self, String> {
        let device = &gpu.device;
        let meshes = checked(device, "creating the mesh pipelines", || {
            let vertex_attributes = wgpu::vertex_attr_array![
                0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32
            ];
            let instance_attributes = wgpu::vertex_attr_array![
                4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
            ];
            let buffers = [
                Some(wgpu::VertexBufferLayout {
                    array_stride: model::VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attributes,
                }),
                // Each flyer is three instances in a row: the model matrices
                // of the body, the door and the turntable.
                Some(wgpu::VertexBufferLayout {
                    array_stride: model::INSTANCE_BYTES / 3,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &instance_attributes,
                }),
            ];
            let pipeline =
                |label: &str, entry: &str, depth_write: bool, blend: Option<wgpu::BlendState>| {
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(label),
                        layout: Some(&scene.pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &scene.shader,
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
                        multisample: wgpu::MultisampleState::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &scene.shader,
                            entry_point: Some(entry),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: config.format,
                                blend,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    })
                };
            let solid = pipeline("solid", "fs_solid", true, None);
            let cutout = pipeline("cutout", "fs_cutout", true, None);
            // resvg renders premultiplied alpha.
            let glass = pipeline(
                "glass",
                "fs_glass",
                false,
                Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            );
            let globals = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mesh globals"),
                size: GLOBALS_BYTES,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mesh globals"),
                layout: &scene.globals_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                }],
            });
            Ok(Self {
                solid,
                cutout,
                glass,
                depth: depth_view(device, config.width, config.height),
                globals,
                globals_group,
                instances: instance_buffer(device, INITIAL_FLYERS),
                bytes: Vec::new(),
            })
        })?;
        meshes.write_globals(gpu, config.width, config.height);
        Ok(meshes)
    }

    /// Fits the depth buffer and the projection to a new surface size.
    fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) -> Result<(), String> {
        self.depth = checked(&gpu.device, "creating the depth buffer", || {
            Ok(depth_view(&gpu.device, width, height))
        })?;
        self.write_globals(gpu, width, height);
        Ok(())
    }

    fn write_globals(&self, gpu: &Gpu, width: u32, height: u32) {
        let projection = flight::projection(width as f32 / height as f32);
        let light = model::light_dir();
        let mut bytes = Vec::with_capacity(GLOBALS_BYTES as usize);
        let values = projection.0.into_iter().flatten().chain([
            light.x,
            light.y,
            light.z,
            0.0,
            model::AMBIENT,
            model::DIFFUSE,
            0.0,
            0.0,
        ]);
        for v in values {
            bytes.extend_from_slice(&v.to_ne_bytes());
        }
        gpu.queue.write_buffer(&self.globals, 0, &bytes);
    }

    /// Writes the instances of `flyers`.
    fn prepare(&mut self, gpu: &Gpu, flyers: &[FlyerInstance]) {
        self.bytes.clear();
        for f in flyers {
            model::write_instance(f, &mut self.bytes);
        }
        let needed = self.bytes.len() as u64;
        if needed > self.instances.size() {
            let count = (needed / model::INSTANCE_BYTES).next_power_of_two();
            self.instances = instance_buffer(&gpu.device, count);
        }
        if needed > 0 {
            gpu.queue.write_buffer(&self.instances, 0, &self.bytes);
        }
    }

    /// Records the draws of `flyers` (already written by `prepare`).
    fn record(&self, scene: &Scene, pass: &mut wgpu::RenderPass<'_>, flyers: &[FlyerInstance]) {
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_bind_group(1, &scene.atlas, &[]);
        pass.set_vertex_buffer(0, scene.vertices.slice(..));
        pass.set_vertex_buffer(1, self.instances.slice(..));
        pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_pipeline(&self.solid);
        for (i, f) in flyers.iter().enumerate() {
            let base = 3 * i as u32;
            let body = scene.body[usize::from(f.color) % scene.body.len()].clone();
            pass.draw_indexed(body, 0, base..base + 1);
            pass.draw_indexed(scene.turntable.clone(), 0, base + 2..base + 3);
        }
        pass.set_pipeline(&self.cutout);
        for i in 0..flyers.len() {
            let base = 3 * i as u32;
            pass.draw_indexed(scene.door.clone(), 0, base + 1..base + 2);
        }
        pass.set_pipeline(&self.glass);
        for i in model::glass_order(flyers) {
            let base = 3 * i as u32;
            pass.draw_indexed(scene.door.clone(), 0, base + 1..base + 2);
        }
    }
}

fn depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

fn instance_buffer(device: &wgpu::Device, flyers: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mesh instances"),
        size: flyers * model::INSTANCE_BYTES,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A configured surface for one window, with what it draws the scene with.
pub struct Target {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// None when the scene or the pipelines could not be made: only the
    /// background is drawn.
    meshes: Option<Meshes>,
}

impl Target {
    /// Configures `surface` for the window size.
    pub fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        size: PhysicalSize<u32>,
    ) -> Result<Self, String> {
        if size.width == 0 || size.height == 0 {
            return Err(format!("window size is {size:?}"));
        }
        let capabilities = surface.get_capabilities(&gpu.adapter);
        log!(
            "surface capabilities: formats {:?}, present modes {:?}, usages {:?}",
            capabilities.formats,
            capabilities.present_modes,
            capabilities.usages
        );
        // Why: a non-sRGB format writes the colors as they are; an sRGB
        // format would brighten the dark background.
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("the surface supports no formats")?;
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let config = wgpu::SurfaceConfiguration {
            usage,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width,
            height: size.height,
            // Fifo is vsync and the only mode guaranteed to be supported.
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
        };
        log!(
            "configuring the surface: {}x{} {format:?} {usage:?}",
            config.width,
            config.height
        );
        surface.configure(&gpu.device, &config);
        log!(
            "surface configured: {}x{} {format:?} {:?}",
            config.width,
            config.height,
            config.present_mode
        );
        let meshes = gpu
            .scene()
            .and_then(|scene| match Meshes::new(gpu, scene, &config) {
                Ok(meshes) => {
                    log!("mesh pipelines ready: {}x{}", config.width, config.height);
                    Some(meshes)
                }
                Err(e) => {
                    log!("{e}: drawing the background only");
                    None
                }
            });
        Ok(Self {
            surface,
            config,
            meshes,
        })
    }

    /// Reconfigures the surface for a new window size (ignored while it is zero).
    pub fn resize(&mut self, gpu: &Gpu, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        if (size.width, size.height) == (self.config.width, self.config.height) {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&gpu.device, &self.config);
        if let Some(meshes) = &mut self.meshes
            && let Err(e) = meshes.resize(gpu, size.width, size.height)
        {
            log!("{e}: drawing the background only");
            self.meshes = None;
        }
    }

    /// Draws one frame. Returns why the frame was skipped, if it was.
    pub fn draw(&mut self, gpu: &Gpu, flyers: &[FlyerInstance]) -> Frame {
        use wgpu::CurrentSurfaceTexture as Acquired;

        let (texture, suboptimal) = match self.surface.get_current_texture() {
            Acquired::Success(texture) => (texture, false),
            // Usable now; reconfigured after presenting (wgpu panics if the
            // surface is configured while a surface texture is alive).
            Acquired::Suboptimal(texture) => (texture, true),
            Acquired::Outdated => {
                self.surface.configure(&gpu.device, &self.config);
                return Frame::Skipped(diag::Skip::Outdated);
            }
            Acquired::Lost => {
                self.surface.configure(&gpu.device, &self.config);
                return Frame::Skipped(diag::Skip::Lost);
            }
            Acquired::Timeout => return Frame::Skipped(diag::Skip::Timeout),
            Acquired::Occluded => return Frame::Skipped(diag::Skip::Occluded),
            Acquired::Validation => return Frame::Skipped(diag::Skip::Validation),
        };

        // `meshes` is only `Some` when the scene was made.
        let scene = gpu.scene.get().and_then(Option::as_ref);
        let drawing = match (&mut self.meshes, scene) {
            (Some(meshes), Some(scene)) if !flyers.is_empty() => {
                meshes.prepare(gpu, flyers);
                Some((&*meshes, scene))
            }
            _ => None,
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        let [r, g, b] = model::BACKGROUND_RGB.map(|c| f64::from(c) / 255.0);
        {
            // Dropping the pass at the end of this block ends it.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: drawing.map(|(meshes, _)| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: &meshes.depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some((meshes, scene)) = drawing {
                meshes.record(scene, &mut pass, flyers);
            }
        }
        gpu.queue.submit([encoder.finish()]);
        drop(view);
        gpu.queue.present(texture);
        if suboptimal {
            self.surface.configure(&gpu.device, &self.config);
        }
        Frame::Presented
    }
}

/// The result of `Target::draw`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    Presented,
    Skipped(diag::Skip),
}

/// Ends the process if GPU initialization hangs. Dropping it disarms it.
/// `/s` and `/p` pass only the exit (plan 0005, D-1 (6)): they show nothing
/// and end at once.
pub struct Watchdog {
    limit: Option<gpu_limit::Limit>,
}

impl Watchdog {
    /// Ends the process with `exit_code` unless dropped within `timeout`.
    pub fn start(timeout: Duration, exit_code: u32) -> Self {
        let started = gpu_limit::Limit::start(timeout, move || {
            log!("GPU initialization timed out; exiting");
            crate::win::terminate_now(exit_code);
        });
        match started {
            Ok(limit) => Self { limit: Some(limit) },
            Err(e) => {
                log!("watchdog thread could not start: {e}");
                Self { limit: None }
            }
        }
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        if let Some(limit) = &self.limit {
            limit.disarm();
        }
    }
}
