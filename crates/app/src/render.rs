//! wgpu setup and the sprite renderer.
//!
//! Each frame clears the surface to the background color and then draws every
//! flyer as two textured quads, the body and the door, in one instanced draw
//! ([`crate::sprite`], `sprite.wgsl`). The textures are the SVG assets,
//! rasterized into one atlas when the surface is configured and again only
//! when the sprite side in pixels changes, never per frame (ADR-0023).
//!
//! Hardening (docs/design/architecture.md):
//! - SEC-2: the DX12 shader compiler is set to `Fxc` explicitly, and the
//!   backend is DX12 only.
//! - The instance is built without `with_env()`, so `WGPU_*` environment
//!   variables cannot change the backend or the compiler.
//! - Power preference is LowPower (do not wake a discrete GPU needlessly).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use flying_microwave_core::sim::SpriteInstance;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::diag;
use crate::log::log;
use crate::sprite::{self, AtlasLayout};

/// Adapter, device and queue shared by every window.
pub struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
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

/// The sprite atlas texture of one surface size.
struct Atlas {
    layout: AtlasLayout,
    bind_group: wgpu::BindGroup,
}

/// What draws the sprites: the pipeline, the atlas and the instance buffer.
struct Sprites {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// None when the atlas could not be made: only the background is drawn.
    atlas: Option<Atlas>,
    /// Body side in pixels the atlas was made for.
    side: f32,
    instances: wgpu::Buffer,
    /// Instance bytes of the current frame, kept to reuse the allocation.
    bytes: Vec<u8>,
}

impl Sprites {
    /// Creates the pipeline for surfaces of `format`.
    fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Result<Self, String> {
        let device = &gpu.device;
        checked(device, "creating the sprite pipeline", || {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("sprite shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("sprite.wgsl").into()),
            });
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("sprite atlas"),
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
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("sprite"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
            let attributes = wgpu::vertex_attr_array![
                0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x2, 4 => Float32x2
            ];
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sprite"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: sprite::GPU_QUAD_BYTES,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &attributes,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    // Tilted and mirrored quads must not be culled.
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        // resvg renders premultiplied alpha.
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("sprite atlas"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            Ok(Self {
                pipeline,
                bind_group_layout,
                sampler,
                atlas: None,
                side: 0.0,
                instances: Self::instance_buffer(device, INITIAL_INSTANCES),
                bytes: Vec::new(),
            })
        })
    }

    fn instance_buffer(device: &wgpu::Device, count: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite instances"),
            size: count * sprite::GPU_QUAD_BYTES,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Rasterizes the atlas again when the body side in pixels changes.
    fn fit(&mut self, gpu: &Gpu, sprite_size: f32, height: u32) {
        let side = sprite_size * height as f32;
        if self.atlas.is_some() && side == self.side {
            return;
        }
        self.side = side;
        let layout = sprite::atlas_layout(side, gpu.device.limits().max_texture_dimension_2d);
        let started = Instant::now();
        self.atlas = match self.make_atlas(gpu, layout) {
            Ok(atlas) => {
                log!(
                    "sprite atlas: {}x{} in {} ms",
                    layout.width,
                    layout.height,
                    started.elapsed().as_millis()
                );
                Some(atlas)
            }
            Err(e) => {
                log!("{e}: drawing the background only");
                None
            }
        };
    }

    fn make_atlas(&self, gpu: &Gpu, layout: AtlasLayout) -> Result<Atlas, String> {
        let pixels = sprite::rasterize(&layout)?;
        checked(&gpu.device, "creating the sprite atlas", || {
            let size = wgpu::Extent3d {
                width: layout.width,
                height: layout.height,
                depth_or_array_layers: 1,
            };
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("sprite atlas"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Why not sRGB: the surface is not sRGB either, so the
                // colors of the SVG reach the screen unchanged.
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(layout.width * 4),
                    rows_per_image: None,
                },
                size,
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sprite atlas"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            Ok(Atlas { layout, bind_group })
        })
    }

    /// Writes the instances of `sprites` and returns how many quads to draw.
    fn prepare(&mut self, gpu: &Gpu, sprites: &[SpriteInstance], width: u32, height: u32) -> u32 {
        let Some(atlas) = &self.atlas else { return 0 };
        self.bytes.clear();
        for s in sprites {
            for quad in sprite::sprite_quads(s, width, height, &atlas.layout) {
                quad.write_to(&mut self.bytes);
            }
        }
        let needed = self.bytes.len() as u64;
        if needed > self.instances.size() {
            let count = (needed / sprite::GPU_QUAD_BYTES).next_power_of_two();
            self.instances = Self::instance_buffer(&gpu.device, count);
        }
        if needed > 0 {
            gpu.queue.write_buffer(&self.instances, 0, &self.bytes);
        }
        (needed / sprite::GPU_QUAD_BYTES) as u32
    }
}

/// Instance buffer capacity at start: two quads (body and door) for 32 flyers.
const INITIAL_INSTANCES: u64 = 64;

/// A configured surface for one window, with the sprite renderer for its size.
pub struct Target {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// None when the sprite pipeline could not be made: only the background is drawn.
    sprites: Option<Sprites>,
    sprite_size: f32,
}

impl Target {
    /// Configures `surface` for the window size. `sprite_size` is the sprite
    /// side in screen heights ([`SpriteInstance::size`]).
    pub fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        size: PhysicalSize<u32>,
        sprite_size: f32,
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
        let sprites = match Sprites::new(gpu, format) {
            Ok(mut sprites) => {
                sprites.fit(gpu, sprite_size, config.height);
                Some(sprites)
            }
            Err(e) => {
                log!("{e}: drawing the background only");
                None
            }
        };
        Ok(Self {
            surface,
            config,
            sprites,
            sprite_size,
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
        if let Some(sprites) = &mut self.sprites {
            sprites.fit(gpu, self.sprite_size, size.height);
        }
    }

    /// Draws one frame. Returns why the frame was skipped, if it was.
    pub fn draw(&mut self, gpu: &Gpu, sprites: &[SpriteInstance]) -> Frame {
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

        let (width, height) = (self.config.width, self.config.height);
        let quads = self
            .sprites
            .as_mut()
            .map_or(0, |s| s.prepare(gpu, sprites, width, height));
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        let [r, g, b] = sprite::BACKGROUND_RGB.map(|c| f64::from(c) / 255.0);
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(s) = &self.sprites
                && let Some(atlas) = &s.atlas
                && quads > 0
            {
                pass.set_pipeline(&s.pipeline);
                pass.set_bind_group(0, &atlas.bind_group, &[]);
                pass.set_vertex_buffer(0, s.instances.slice(..));
                // Instances are drawn in order, so each door covers its body
                // and later flyers cover earlier ones.
                pass.draw(0..4, 0..quads);
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
pub struct Watchdog {
    finished: Arc<AtomicBool>,
}

impl Watchdog {
    /// Ends the process with `exit_code` unless dropped within `timeout`.
    pub fn start(timeout: Duration, exit_code: u32) -> Self {
        let finished = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&finished);
        // A thread, because a hung driver call also blocks the event loop.
        let spawned = std::thread::Builder::new()
            .name("gpu-init-watchdog".to_owned())
            .spawn(move || {
                std::thread::sleep(timeout);
                if !flag.load(Ordering::SeqCst) {
                    log!("GPU initialization timed out; exiting");
                    crate::win::terminate_now(exit_code);
                }
            });
        if let Err(e) = spawned {
            log!("watchdog thread could not start: {e}");
        }
        Self { finished }
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::SeqCst);
    }
}
