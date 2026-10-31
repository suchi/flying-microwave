//! `/p <HWND>` mode (spike S1): a child window of the settings dialog's
//! preview frame, filled by wgpu with the same slowly changing color as `/s`.
//!
//! The process ignores all input and exits as soon as the parent window is
//! gone (checked every 500 ms) or its own window is destroyed. No dialog is
//! ever shown here.

use std::num::NonZeroIsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use winit::window::{Window, WindowId};

use crate::gpu::{self, Frame};
use crate::log::log;
use crate::win;

const PARENT_CHECK_INTERVAL: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_millis(50);
/// Number of ignored input events that are written to the log.
const IGNORED_INPUT_LOGGED: u32 = 10;

const EXIT_OK: i32 = 0;
const EXIT_EVENT_LOOP_FAILED: i32 = 1;
/// The HWND argument was missing, malformed or not a window.
const EXIT_INVALID_HWND: i32 = 2;
const EXIT_INIT_FAILED: i32 = 4;

struct PreviewApp {
    options: gpu::Options,
    parent: usize,
    started: Instant,
    window: Option<Arc<Window>>,
    target: Option<gpu::Target>,
    gpu: Option<gpu::Gpu>,
    instance: Option<wgpu::Instance>,
    next_parent_check: Instant,
    parent_checks: u64,
    retry_at: Option<Instant>,
    frames: u64,
    ignored_input: u32,
    ignored_input_total: u64,
    exiting: bool,
    exit_code: i32,
}

/// Runs the preview and returns the process exit code.
pub fn run(options: gpu::Options, hwnd: Option<usize>) -> i32 {
    let Some(parent) = hwnd else {
        log!(
            "preview: HWND argument missing or not an unsigned decimal number; exit code {EXIT_INVALID_HWND}"
        );
        return EXIT_INVALID_HWND;
    };
    if !win::is_window(win::hwnd_from_usize(parent)) {
        log!(
            "preview: HWND {parent} is not a valid window (IsWindow = FALSE); exit code {EXIT_INVALID_HWND}"
        );
        return EXIT_INVALID_HWND;
    }
    log!("preview: HWND {parent} is a valid window");

    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            log!("preview: EventLoop::new failed: {e}");
            return EXIT_EVENT_LOOP_FAILED;
        }
    };
    let mut app = PreviewApp {
        options,
        parent,
        started: Instant::now(),
        window: None,
        target: None,
        gpu: None,
        instance: None,
        next_parent_check: Instant::now() + PARENT_CHECK_INTERVAL,
        parent_checks: 0,
        retry_at: None,
        frames: 0,
        ignored_input: 0,
        ignored_input_total: 0,
        exiting: false,
        exit_code: EXIT_OK,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        log!("preview: event loop returned an error: {e}");
        return EXIT_EVENT_LOOP_FAILED;
    }
    log!(
        "preview: event loop finished; {} frames, {} ignored input events",
        app.frames,
        app.ignored_input_total
    );
    app.exit_code
}

impl PreviewApp {
    fn parent_hwnd(&self) -> HWND {
        win::hwnd_from_usize(self.parent)
    }

    fn finish(&mut self, event_loop: &ActiveEventLoop, code: i32, reason: &str) {
        if self.exiting {
            return;
        }
        self.exiting = true;
        self.exit_code = code;
        log!("EXIT requested: {reason} (exit code {code})");
        event_loop.exit();
    }

    /// Creates the child window and the GPU resources. Returns an error text on failure.
    fn setup(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let parent = self.parent_hwnd();
        // Re-validate right before use: the dialog may have gone away already.
        if !win::is_window(parent) {
            return Err("parent window disappeared before the child was created".to_owned());
        }
        let (width, height) = win::client_size(parent)
            .map_err(|code| format!("GetClientRect failed, GetLastError={code}"))?;
        log!("parent client rectangle: {width}x{height}");
        let size = PhysicalSize::new(
            u32::try_from(width).unwrap_or(0).max(1),
            u32::try_from(height).unwrap_or(0).max(1),
        );

        let handle = NonZeroIsize::new(self.parent as isize)
            .ok_or_else(|| "parent HWND is zero".to_owned())?;
        let attributes = Window::default_attributes()
            .with_title("saver-spike preview")
            .with_decorations(false)
            .with_resizable(false)
            // The preview must never take the focus away from the dialog.
            .with_active(false)
            .with_position(PhysicalPosition::new(0, 0))
            .with_inner_size(size);
        // SAFETY: the handle was validated with IsWindow just above. If the
        // parent is destroyed right after, window creation fails or the child
        // is destroyed with it, which this mode handles.
        let attributes = unsafe {
            attributes
                .with_parent_window(Some(RawWindowHandle::Win32(Win32WindowHandle::new(handle))))
        };
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| format!("create_window failed: {e}"))?,
        );
        let hwnd = win::hwnd_of(&window);
        let black = hwnd.map_or("no HWND", win::fill_black);
        log!(
            "child window created: id={:?} hwnd={hwnd:?} inner_size={:?} outer_position={:?} \
             scale_factor={} requested_size={size:?}, black fill: {black}",
            window.id(),
            window.inner_size(),
            window.outer_position(),
            window.scale_factor(),
        );

        let (instance, gpu, target) = gpu::guarded("GPU initialization", || {
            let instance = gpu::create_instance(&self.options);
            let surface = gpu::create_surface(&instance, &window)?;
            let gpu = gpu::request_gpu(&instance, &self.options, Some(&surface))?;
            let target = gpu::Target::new(&gpu, surface, window.inner_size(), "preview")?;
            Ok((instance, gpu, target))
        })?;
        self.window = Some(window);
        self.instance = Some(instance);
        self.gpu = Some(gpu);
        self.target = Some(target);
        Ok(())
    }

    fn redraw(&mut self) {
        let (Some(window), Some(target), Some(gpu)) = (
            self.window.as_ref(),
            self.target.as_mut(),
            self.gpu.as_ref(),
        ) else {
            return;
        };
        match target.draw(gpu, self.started.elapsed()) {
            Frame::Presented => {
                self.frames += 1;
                if self.frames <= 3 {
                    log!("preview: frame {} presented", self.frames);
                }
                window.request_redraw();
            }
            Frame::Skipped => self.retry_at = Some(Instant::now() + RETRY_DELAY),
        }
    }

    fn ignore_input(&mut self, what: &str) {
        self.ignored_input_total += 1;
        if self.ignored_input < IGNORED_INPUT_LOGGED {
            self.ignored_input += 1;
            log!("input ignored in preview mode: {what}");
        } else if self.ignored_input == IGNORED_INPUT_LOGGED {
            self.ignored_input += 1;
            log!("further ignored input events are only counted");
        }
    }
}

impl ApplicationHandler for PreviewApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.exiting {
            return;
        }
        match self.setup(event_loop) {
            Ok(()) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_parent_check));
            }
            Err(e) => {
                // Preview failures end quietly (no dialog), as designed.
                log!("preview: setup FAILED: {e}");
                self.finish(event_loop, EXIT_INIT_FAILED, "setup failed");
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Destroyed => {
                log!("preview: WindowEvent::Destroyed");
                self.finish(event_loop, EXIT_OK, "own window destroyed");
            }
            WindowEvent::CloseRequested => {
                log!("preview: WindowEvent::CloseRequested");
                self.finish(event_loop, EXIT_OK, "close requested");
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(size) => {
                log!("preview: Resized {size:?}");
                if let (Some(target), Some(gpu)) = (self.target.as_mut(), self.gpu.as_ref()) {
                    target.resize(gpu, size);
                }
            }
            WindowEvent::Moved(position) => log!("preview: Moved {position:?}"),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                log!("preview: ScaleFactorChanged scale_factor={scale_factor}");
            }
            WindowEvent::Focused(focused) => log!("preview: Focused({focused})"),
            WindowEvent::Occluded(occluded) => log!("preview: Occluded({occluded})"),
            WindowEvent::KeyboardInput { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::Touch(_)
            | WindowEvent::CursorMoved { .. } => self.ignore_input(&format!("{event:?}")),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.exiting {
            return;
        }
        let now = Instant::now();
        if now >= self.next_parent_check {
            self.next_parent_check = now + PARENT_CHECK_INTERVAL;
            self.parent_checks += 1;
            if win::is_window(self.parent_hwnd()) {
                if self.parent_checks == 1 || self.parent_checks.is_multiple_of(20) {
                    log!(
                        "preview: parent still valid (check #{})",
                        self.parent_checks
                    );
                }
            } else {
                log!("preview: parent HWND {} is no longer a window", self.parent);
                self.finish(event_loop, EXIT_OK, "parent window is gone");
                return;
            }
        }
        let mut wake = self.next_parent_check;
        if let Some(at) = self.retry_at {
            if at <= now {
                self.retry_at = None;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            } else {
                wake = wake.min(at);
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        log!("preview: exiting: dropping surface and window");
        self.target = None;
        self.gpu = None;
        self.instance = None;
        self.window = None;
    }
}
