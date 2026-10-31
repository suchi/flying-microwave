//! wgpu setup and a trivial "clear to a color" renderer.
//!
//! The hardening decisions mirror docs/design/architecture.md:
//! - SEC-2: the DX12 shader compiler is set to `Fxc` explicitly (the default,
//!   `Auto`, would try to load `dxcompiler.dll` first).
//! - The backend is chosen explicitly (DX12 only by default).
//! - The instance is built without `with_env()`, so `WGPU_*` environment
//!   variables cannot change the backend or compiler.
//! - Power preference is LowPower (do not wake a discrete GPU).

use std::sync::Arc;
use std::time::Duration;

use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::log::log;
use crate::policy;

/// Choices read from the flag files next to the executable (or fixed by a mode).
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Ask for the fallback (software) adapter: WARP on DX12.
    pub force_fallback: bool,
    /// Use Vulkan instead of DX12.
    pub vulkan: bool,
}

/// Adapter, device and queue. The instance is kept by the caller because
/// surfaces must be created before the adapter is requested.
pub struct Gpu {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// What happened when a frame was attempted.
#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// A frame was drawn and presented.
    Presented,
    /// The frame was skipped (occluded, timeout, outdated surface, ...).
    Skipped,
}

/// A configured presentation surface for one window.
pub struct Target {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    label: String,
    /// Number of non-success acquire results seen (used to limit log noise).
    problems: u32,
}

/// Creates the wgpu instance with the explicit backend and shader compiler.
pub fn create_instance(options: &Options) -> wgpu::Instance {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = if options.vulkan {
        wgpu::Backends::VULKAN
    } else {
        wgpu::Backends::DX12
    };
    descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
    log!(
        "wgpu instance: options={options:?} backends={:?} dx12.shader_compiler=Fxc flags={:?} \
         compiled-in backends={:?}",
        descriptor.backends,
        descriptor.flags,
        wgpu::Instance::enabled_backend_features(),
    );
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

/// Requests the adapter and device. `surface` makes the adapter prove that it
/// can present to a real window. Blocks until both futures complete.
///
/// Logs the chosen adapter's `AdapterInfo` (spike S6).
pub fn request_gpu(
    instance: &wgpu::Instance,
    options: &Options,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<Gpu, String> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: options.force_fallback,
        compatible_surface: surface,
        apply_limit_buckets: false,
    }))
    .map_err(|e| format!("request_adapter failed: {e}"))?;
    log!("adapter chosen: {:?}", adapter.get_info());

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("saver-spike device"),
        // Why: ask for exactly what the adapter offers, so that large (4K)
        // surfaces are never rejected by conservative default limits.
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| format!("request_device failed: {e}"))?;

    // By default wgpu panics on uncaptured errors; log them instead.
    device.on_uncaptured_error(Arc::new(|error| {
        log!("wgpu uncaptured error: {error}");
    }));
    device.set_device_lost_callback(|reason, message| {
        log!("wgpu device lost: reason={reason:?} message={message}");
    });
    Ok(Gpu {
        adapter,
        device,
        queue,
    })
}

/// Runs GPU initialization and turns a panic inside wgpu into an `Err`, so
/// that callers can apply their "initialization failed" policy.
pub fn guarded<T>(what: &str, init: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(init)) {
        Ok(result) => result,
        Err(_) => Err(format!("{what} panicked (see PANIC line above)")),
    }
}

impl Target {
    /// Configures `surface` for the given window size.
    pub fn new(
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        size: PhysicalSize<u32>,
        label: &str,
    ) -> Result<Self, String> {
        if size.width == 0 || size.height == 0 {
            return Err(format!("{label}: window size is {size:?}"));
        }
        let capabilities = surface.get_capabilities(&gpu.adapter);
        log!(
            "{label}: surface capabilities: formats={:?} present_modes={:?} alpha_modes={:?}",
            capabilities.formats,
            capabilities.present_modes,
            capabilities.alpha_modes,
        );
        // Why: prefer a non-sRGB format so that the clear color is written as
        // is; an sRGB format would brighten the dark color.
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or_else(|| format!("{label}: surface supports no formats"))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
        surface.configure(&gpu.device, &config);
        log!(
            "{label}: surface configured: {}x{} format={:?} present_mode={:?}",
            config.width,
            config.height,
            config.format,
            config.present_mode,
        );
        Ok(Self {
            surface,
            config,
            label: label.to_owned(),
            problems: 0,
        })
    }

    /// Reconfigures the surface for a new size (ignored while the size is zero).
    pub fn resize(&mut self, gpu: &Gpu, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            log!("{}: resize to {size:?} ignored", self.label);
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&gpu.device, &self.config);
        log!("{}: surface reconfigured to {size:?}", self.label);
    }

    /// Draws one frame: clears the surface to the dark color for `elapsed`.
    pub fn draw(&mut self, gpu: &Gpu, elapsed: Duration) -> Frame {
        use wgpu::CurrentSurfaceTexture as Acquired;

        let (texture, suboptimal) = match self.surface.get_current_texture() {
            Acquired::Success(texture) => (texture, false),
            // Usable now; the surface is reconfigured after presenting (wgpu
            // panics if configure is called while a surface texture is alive).
            Acquired::Suboptimal(texture) => (texture, true),
            Acquired::Outdated => {
                self.note_problem("Outdated (reconfiguring)");
                self.surface.configure(&gpu.device, &self.config);
                return Frame::Skipped;
            }
            Acquired::Lost => {
                self.note_problem("Lost (reconfiguring; a real fix would recreate the surface)");
                self.surface.configure(&gpu.device, &self.config);
                return Frame::Skipped;
            }
            Acquired::Timeout => {
                self.note_problem("Timeout");
                return Frame::Skipped;
            }
            Acquired::Occluded => {
                self.note_problem("Occluded");
                return Frame::Skipped;
            }
            Acquired::Validation => {
                self.note_problem("Validation");
                return Frame::Skipped;
            }
        };

        let [r, g, b] = policy::dark_color(elapsed);
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("saver-spike encoder"),
            });
        {
            // Dropping the pass at the end of this block ends it.
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
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
        }
        gpu.queue.submit([encoder.finish()]);
        // Release our view of the texture before presenting it.
        drop(view);
        gpu.queue.present(texture);
        if suboptimal {
            self.note_problem("Suboptimal (reconfiguring after present)");
            self.surface.configure(&gpu.device, &self.config);
        }
        Frame::Presented
    }

    /// Logs the first few non-success acquire results, then goes quiet.
    fn note_problem(&mut self, what: &str) {
        self.problems += 1;
        if self.problems <= 10 {
            log!(
                "{}: get_current_texture returned {what} (occurrence {})",
                self.label,
                self.problems
            );
        } else if self.problems == 11 {
            log!("{}: further acquire problems are not logged", self.label);
        }
    }
}
