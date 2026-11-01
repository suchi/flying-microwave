//! wgpu setup and the placeholder renderer (Phase 1, task 5).
//!
//! Each frame clears the surface to the background color and then copies the
//! solid rectangles of every sprite onto it ([`crate::layout`]). Copying
//! needs no shader, so the first runnable version needs no shader compiler.
//!
//! Hardening (docs/design/architecture.md):
//! - SEC-2: the DX12 shader compiler is set to `Fxc` explicitly, and the
//!   backend is DX12 only.
//! - The instance is built without `with_env()`, so `WGPU_*` environment
//!   variables cannot change the backend or the compiler.
//! - Power preference is LowPower (do not wake a discrete GPU needlessly).

use std::sync::Arc;

use flying_microwave_core::sim::SpriteInstance;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::layout::{self, PARTS, Part};
use crate::log::log;

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
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("flying-microwave device"),
        // Why: ask for what the adapter offers, so large (4K) surfaces are
        // never rejected by conservative default limits.
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| format!("request_device failed: {e}"))?;
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

/// The pixels of one sprite part, ready to copy onto the surface.
struct PartPixels {
    part: Part,
    buffer: wgpu::Buffer,
    bytes_per_row: u32,
}

/// A configured surface for one window, with the sprite pixels for its size.
pub struct Target {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// Empty when the surface cannot take copies: only the background is drawn.
    parts: Vec<PartPixels>,
    side: u32,
    sprite_size: f32,
    /// Byte order of one pixel, or None when the format is not 8-bit RGBA/BGRA.
    bgra: Option<bool>,
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
        // Why: a non-sRGB format writes the colors as they are; an sRGB
        // format would brighten the dark background.
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("the surface supports no formats")?;
        let bgra = match format {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => Some(true),
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => Some(false),
            _ => None,
        };
        // Copies need COPY_DST on the surface texture (DX12 offers it).
        let can_copy =
            capabilities.usages.contains(wgpu::TextureUsages::COPY_DST) && bgra.is_some();
        let usage = if can_copy {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST
        } else {
            log!("surface cannot take copies ({format:?}): drawing the background only");
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };
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
        surface.configure(&gpu.device, &config);
        log!(
            "surface configured: {}x{} {format:?} {:?}",
            config.width,
            config.height,
            config.present_mode
        );
        let mut target = Self {
            surface,
            config,
            parts: Vec::new(),
            side: 0,
            sprite_size,
            bgra: can_copy.then_some(bgra).flatten(),
        };
        target.rebuild_parts(gpu);
        Ok(target)
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
        self.rebuild_parts(gpu);
    }

    /// Recreates the part pixels when the sprite side in pixels changes.
    fn rebuild_parts(&mut self, gpu: &Gpu) {
        let Some(bgra) = self.bgra else { return };
        let side = layout::side_px(self.sprite_size, self.config.height);
        if side == self.side && !self.parts.is_empty() {
            return;
        }
        self.side = side;
        self.parts = PARTS
            .iter()
            .map(|&part| {
                let (w, h) = layout::part_size(part, side);
                let [r, g, b] = part.rgb();
                let pixel = if bgra { [b, g, r, 255] } else { [r, g, b, 255] };
                // Why: buffer-to-texture copies need rows aligned to 256 bytes.
                let bytes_per_row = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
                    * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                let row: Vec<u8> = pixel
                    .iter()
                    .copied()
                    .cycle()
                    .take(bytes_per_row as usize)
                    .collect();
                let buffer = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sprite part"),
                        contents: &row.repeat(h as usize),
                        usage: wgpu::BufferUsages::COPY_SRC,
                    });
                PartPixels {
                    part,
                    buffer,
                    bytes_per_row,
                }
            })
            .collect();
    }

    /// Draws one frame. Returns false when the frame was skipped.
    pub fn draw(&mut self, gpu: &Gpu, sprites: &[SpriteInstance]) -> bool {
        use wgpu::CurrentSurfaceTexture as Acquired;

        let (texture, suboptimal) = match self.surface.get_current_texture() {
            Acquired::Success(texture) => (texture, false),
            // Usable now; reconfigured after presenting (wgpu panics if the
            // surface is configured while a surface texture is alive).
            Acquired::Suboptimal(texture) => (texture, true),
            Acquired::Outdated | Acquired::Lost => {
                self.surface.configure(&gpu.device, &self.config);
                return false;
            }
            Acquired::Timeout | Acquired::Occluded | Acquired::Validation => return false,
        };

        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        let [r, g, b] = layout::BACKGROUND_RGB.map(|c| f64::from(c) / 255.0);
        {
            // Dropping the pass at the end of this block ends it.
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("background"),
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
        // Recorded after the background pass, so the sprites are drawn on top.
        let (width, height) = (self.config.width, self.config.height);
        for sprite in sprites {
            let center = layout::center_px(sprite, height);
            for pixels in &self.parts {
                let rect = layout::part_rect(pixels.part, center, self.side);
                let Some(copy) = layout::clip(rect, width, height) else {
                    continue;
                };
                encoder.copy_buffer_to_texture(
                    wgpu::TexelCopyBufferInfo {
                        buffer: &pixels.buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            // A multiple of the 4-byte pixel, as copies require.
                            offset: u64::from(copy.src_y) * u64::from(pixels.bytes_per_row)
                                + u64::from(copy.src_x) * 4,
                            bytes_per_row: Some(pixels.bytes_per_row),
                            rows_per_image: None,
                        },
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: copy.dst_x,
                            y: copy.dst_y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: copy.w,
                        height: copy.h,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        gpu.queue.submit([encoder.finish()]);
        drop(view);
        gpu.queue.present(texture);
        if suboptimal {
            self.surface.configure(&gpu.device, &self.config);
        }
        true
    }
}
