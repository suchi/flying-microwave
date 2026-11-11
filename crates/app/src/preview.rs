//! `/p <HWND>`: the small preview inside the Screen Saver Settings dialog
//! (plan 0001, Phase 1 task 7; spike S1 in docs/research/spike-results.md).
//!
//! A child window fills the client area of the dialog's preview frame and
//! shows the same flying microwaves as `/s`. All input is ignored. The
//! process ends when its window is closed or destroyed, or when the parent
//! window is no longer valid (checked every 500 ms). The same check follows
//! the size of the parent's client area. No dialog is ever shown here: a
//! failure ends quietly with an exit code.
//!
//! The settings `flight.*` and `display.max_fps` apply (plan 0004, F-06);
//! the monitors and the mouse sensitivity do not, as the preview is one
//! child window that ignores input.

use std::num::NonZeroIsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use flying_microwave_core::exit_policy::{Decision, Event, ExitPolicy, Mode};
use flying_microwave_core::flight::{Style, World};
use flying_microwave_core::pacing::{FramePacer, Pace};
use flying_microwave_core::sim::SimParams;
use windows_sys::Win32::Foundation::HWND;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use winit::window::{Window, WindowId};

use crate::log::log;
use crate::render::{self, Frame, Gpu, Target};
use crate::saver::{aspect_of, seed};
use crate::storage::Startup;
use crate::win;

const PARENT_CHECK_INTERVAL: Duration = Duration::from_millis(500);
const RETRY_DELAY: Duration = Duration::from_millis(50);
/// Same limit as `/s`: a hung driver call must not leave the process behind.
const GPU_INIT_TIMEOUT: Duration = Duration::from_secs(15);

const EXIT_OK: i32 = 0;
const EXIT_EVENT_LOOP_FAILED: i32 = 1;
/// The HWND argument is not a valid window (architecture: "exit quietly
/// with an error code").
const EXIT_INVALID_PARENT: i32 = 2;
const EXIT_INIT_FAILED: i32 = 4;
/// Same code as `/s` uses for a GPU initialization that does not finish.
const EXIT_GPU_INIT_TIMEOUT: u32 = 6;

struct PreviewApp {
    parent: HWND,
    started: Instant,
    policy: ExitPolicy,
    window: Option<Arc<Window>>,
    world: Option<World>,
    aspect: f32,
    params: SimParams,
    /// `flight.style` of the settings.
    style: Style,
    /// `display.max_fps`, until the window's monitor is known.
    max_fps: u32,
    pacer: Option<FramePacer>,
    gpu: Option<Gpu>,
    target: Option<Target>,
    last_frame: Option<Instant>,
    next_parent_check: Instant,
    retry_at: Option<Instant>,
    frames: u64,
    exiting: bool,
    exit_code: i32,
}

/// Runs the preview in the window `parent` and returns the exit code.
pub fn run(parent: u64) -> i32 {
    let hwnd = win::hwnd_from_arg(parent);
    // SEC-3: the handle comes from the command line.
    if !win::is_window(hwnd) {
        log!("preview: {parent} is not a valid window; exiting");
        return EXIT_INVALID_PARENT;
    }
    // Why here: the preview covers no screen, so reading first delays nothing
    // that matters (plan 0004, F-06 is about `/s`).
    let startup = Startup::load();
    log!("{}", startup.summary());
    for note in &startup.loaded.notes {
        log!("config: {note}");
    }
    let config = startup.loaded.config;
    log!("settings: {config:?}");
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            log!("preview: EventLoop::new failed: {e}");
            return EXIT_EVENT_LOOP_FAILED;
        }
    };
    let mut app = PreviewApp {
        parent: hwnd,
        started: Instant::now(),
        policy: ExitPolicy::new(
            Mode::Preview,
            ExitPolicy::DEFAULT_THRESHOLD_PX,
            ExitPolicy::DEFAULT_GRACE,
        ),
        window: None,
        world: None,
        aspect: 1.0,
        params: config.sim_params(),
        style: config.flight.style,
        max_fps: config.display.max_fps.value(),
        pacer: None,
        gpu: None,
        target: None,
        last_frame: None,
        next_parent_check: Instant::now() + PARENT_CHECK_INTERVAL,
        retry_at: None,
        frames: 0,
        exiting: false,
        exit_code: EXIT_OK,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        log!("preview: event loop returned an error: {e}");
        return EXIT_EVENT_LOOP_FAILED;
    }
    log!("preview ended: {} frames", app.frames);
    app.exit_code
}

impl PreviewApp {
    fn decide(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        if self.policy.handle(event, self.started.elapsed()) == Decision::Exit {
            self.finish(event_loop, EXIT_OK, &format!("{event:?}"));
        }
    }

    fn finish(&mut self, event_loop: &ActiveEventLoop, code: i32, reason: &str) {
        if self.exiting {
            return;
        }
        self.exiting = true;
        self.exit_code = code;
        log!("preview exit: {reason} (exit code {code})");
        event_loop.exit();
    }

    /// Creates the child window, the simulation and the GPU resources.
    fn setup(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let (width, height) =
            win::client_size(self.parent).ok_or("GetClientRect failed on the parent")?;
        let size = PhysicalSize::new(width.max(1), height.max(1));
        let handle =
            NonZeroIsize::new(self.parent as isize).ok_or("the parent window handle is zero")?;
        let attributes = Window::default_attributes()
            .with_title("Flying Microwave preview")
            .with_decorations(false)
            .with_resizable(false)
            // The preview must never take the focus away from the dialog.
            .with_active(false)
            .with_position(PhysicalPosition::new(0, 0))
            .with_inner_size(size);
        // SAFETY: the handle passed is_window just above. If the parent is
        // destroyed right after, window creation fails or the child is
        // destroyed with it; both end the preview.
        let attributes = unsafe {
            attributes
                .with_parent_window(Some(RawWindowHandle::Win32(Win32WindowHandle::new(handle))))
        };
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| format!("create_window failed: {e}"))?,
        );
        // W-9: cover the frame before any GPU work.
        if let Some(hwnd) = win::hwnd_of(&window) {
            win::fill_black(hwnd);
        }
        let inner = window.inner_size();
        log!("preview window: parent client {width}x{height}, window {inner:?}");

        let aspect = aspect_of(inner);
        let world = World::new(seed(0), aspect, self.params, self.style);
        let refresh = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz());
        let pacer = FramePacer::new(self.max_fps, refresh);
        log!("preview: frame rate limited {}", pacer.limits());
        self.pacer = Some(pacer);
        let _watchdog = render::Watchdog::start(GPU_INIT_TIMEOUT, EXIT_GPU_INIT_TIMEOUT);
        let (gpu, target) = render::guarded("GPU initialization", || {
            let instance = render::create_instance();
            let surface = render::create_surface(&instance, &window)?;
            let gpu = render::request_gpu(&instance, &surface)?;
            let target = Target::new(&gpu, surface, inner)?;
            Ok((gpu, target))
        })?;
        self.window = Some(window);
        self.world = Some(world);
        self.aspect = aspect;
        self.gpu = Some(gpu);
        self.target = Some(target);
        Ok(())
    }

    /// W-3: keep covering the whole client area of the parent, which can
    /// change size (for example when the dialog moves to a monitor with
    /// another scale). A child window gets no WM_DPICHANGED of its own.
    fn follow_parent_size(&self) {
        let (Some(window), Some((width, height))) =
            (self.window.as_ref(), win::client_size(self.parent))
        else {
            return;
        };
        let wanted = PhysicalSize::new(width.max(1), height.max(1));
        if window.inner_size() != wanted {
            log!("preview: parent client area is now {width}x{height}");
            // The result arrives as WindowEvent::Resized.
            let _ = window.request_inner_size(wanted);
        }
    }

    fn redraw(&mut self) {
        let (Some(window), Some(world), Some(gpu), Some(target)) = (
            self.window.as_ref(),
            self.world.as_mut(),
            self.gpu.as_ref(),
            self.target.as_mut(),
        ) else {
            return;
        };
        let now = Instant::now();
        // display.max_fps: skip this frame and draw at the given time.
        if let Some(pacer) = self.pacer.as_mut()
            && let Pace::WaitUntil(at) = pacer.poll(now - self.started)
        {
            self.retry_at = Some(self.started + at);
            return;
        }
        if let Some(last) = self.last_frame {
            world.advance(now - last);
        }
        self.last_frame = Some(now);
        if target.draw(gpu, &world.instances()) == Frame::Presented {
            self.frames += 1;
            if self.frames == 1 {
                log!("preview: first frame presented");
            }
            window.request_redraw();
        } else {
            self.retry_at = Some(now + RETRY_DELAY);
        }
    }
}

impl ApplicationHandler for PreviewApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.exiting {
            return;
        }
        // Check again right before use: the dialog may already be gone.
        if !win::is_window(self.parent) {
            log!("preview: the parent window is gone before setup");
            self.finish(event_loop, EXIT_INVALID_PARENT, "parent gone before setup");
            return;
        }
        match self.setup(event_loop) {
            Ok(()) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Err(e) => {
                log!("preview: setup failed: {e}");
                self.finish(event_loop, EXIT_INIT_FAILED, "setup failed");
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.decide(event_loop, Event::CloseRequested),
            WindowEvent::Destroyed => self.decide(event_loop, Event::WindowDestroyed),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(size) => {
                log!("preview: resized to {size:?}");
                if let (Some(target), Some(gpu)) = (self.target.as_mut(), self.gpu.as_ref()) {
                    target.resize(gpu, size);
                }
                // Same rule as `/s`: a new world when the aspect changes.
                let aspect = aspect_of(size);
                if size.height > 0 && (aspect - self.aspect).abs() > 1e-3 {
                    self.world = Some(World::new(seed(0), aspect, self.params, self.style));
                    self.aspect = aspect;
                    self.last_frame = None;
                }
            }
            // Input is ignored in the preview (ExitPolicy, Mode::Preview).
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
            if !win::is_window(self.parent) {
                self.decide(event_loop, Event::ParentGone);
                return;
            }
            self.follow_parent_size();
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
        // Drop the surface before the window it draws into.
        self.target = None;
        self.gpu = None;
        self.window = None;
    }
}
