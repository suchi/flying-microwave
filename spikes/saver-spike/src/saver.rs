//! `/s` mode (spikes S4 and S6): one borderless fullscreen window per monitor
//! in a single event loop, painted black first and then cleared to a slowly
//! changing dark color with wgpu.
//!
//! The spike logs every window event listed in the task so that the focus
//! order, synthetic key events and cursor behavior can be read from the log.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowId, WindowLevel};

use crate::gpu::{self, Frame};
use crate::log::log;
use crate::{policy, win};

/// How long to wait before retrying after a skipped frame (occluded surface and so on).
const RETRY_DELAY: Duration = Duration::from_millis(50);
/// How often the per-window frame rate is written to the log.
const FRAME_REPORT_INTERVAL: Duration = Duration::from_secs(10);
/// Only the first CursorMoved events are logged; later ones would flood the log.
const CURSOR_MOVES_LOGGED: u32 = 20;

/// Exit codes returned to `main`.
const EXIT_OK: i32 = 0;
const EXIT_EVENT_LOOP_FAILED: i32 = 1;
const EXIT_NO_WINDOW: i32 = 5;

struct SaverWindow {
    window: Arc<Window>,
    /// `None` when the GPU failed to initialize: the window then stays black.
    target: Option<gpu::Target>,
    label: String,
    focused: bool,
    frames: u64,
    frames_since_report: u64,
    last_report: Instant,
    /// Set after a skipped frame; the redraw is retried from `about_to_wait`.
    retry_at: Option<Instant>,
    /// Number of GDI black fills logged (limits log noise when there is no GPU).
    black_fills_logged: u32,
}

struct SaverApp {
    options: gpu::Options,
    started: Instant,
    windows: Vec<SaverWindow>,
    // Field order matters little (wgpu keeps its own references), but keep the
    // instance after the targets that were created from it.
    gpu: Option<gpu::Gpu>,
    instance: Option<wgpu::Instance>,
    start_cursor: Option<(i32, i32)>,
    grace_until: Option<Instant>,
    cursor_moves_logged: u32,
    exit_reason: Option<String>,
    exit_code: i32,
    created: bool,
}

/// Runs the screensaver and returns the process exit code.
pub fn run(options: gpu::Options) -> i32 {
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            log!("saver: EventLoop::new failed: {e}");
            return EXIT_EVENT_LOOP_FAILED;
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = SaverApp {
        options,
        started: Instant::now(),
        windows: Vec::new(),
        gpu: None,
        instance: None,
        start_cursor: None,
        grace_until: None,
        cursor_moves_logged: 0,
        exit_reason: None,
        exit_code: EXIT_OK,
        created: false,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        log!("saver: event loop returned an error: {e}");
        return EXIT_EVENT_LOOP_FAILED;
    }
    log!(
        "saver: event loop finished, reason={:?}",
        app.exit_reason.as_deref().unwrap_or("none")
    );
    app.exit_code
}

impl SaverApp {
    fn index_of(&self, id: WindowId) -> Option<usize> {
        self.windows.iter().position(|w| w.window.id() == id)
    }

    fn in_grace(&self) -> bool {
        self.grace_until.is_some_and(|until| Instant::now() < until)
    }

    /// Records the reason and stops the event loop (only the first request counts).
    fn request_exit(&mut self, event_loop: &ActiveEventLoop, reason: &str) {
        if self.exit_reason.is_some() {
            return;
        }
        log!("EXIT requested: {reason}");
        self.exit_reason = Some(reason.to_owned());
        event_loop.exit();
    }

    /// Applies the exit rule to an input event: ignored during the grace
    /// period, otherwise the screensaver ends.
    fn input_decision(&mut self, event_loop: &ActiveEventLoop, what: &str) {
        if self.in_grace() {
            log!("decision: {what} ignored (grace period still running)");
        } else {
            log!("decision: {what} ends the screensaver");
            self.request_exit(event_loop, what);
        }
    }

    fn create_windows(&mut self, event_loop: &ActiveEventLoop) {
        let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
        log!(
            "monitors: {} found, primary={:?}",
            monitors.len(),
            event_loop.primary_monitor().and_then(|m| m.name())
        );
        for (i, monitor) in monitors.iter().enumerate() {
            log!(
                "monitor[{i}] name={:?} position={:?} size={:?} scale_factor={} refresh_mhz={:?}",
                monitor.name(),
                monitor.position(),
                monitor.size(),
                monitor.scale_factor(),
                monitor.refresh_rate_millihertz(),
            );
        }
        // Without any monitor information, fullscreen on the current monitor.
        let targets: Vec<Option<MonitorHandle>> = if monitors.is_empty() {
            vec![None]
        } else {
            monitors.into_iter().map(Some).collect()
        };

        for (index, monitor) in targets.into_iter().enumerate() {
            let attributes = Window::default_attributes()
                .with_title("saver-spike")
                .with_decorations(false)
                // Spike S4 also checks that the windows end up above the taskbar.
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_fullscreen(Some(Fullscreen::Borderless(monitor)));
            let window = match event_loop.create_window(attributes) {
                Ok(window) => Arc::new(window),
                Err(e) => {
                    log!("window[{index}] creation FAILED: {e}");
                    continue;
                }
            };
            window.set_cursor_visible(false);
            let hwnd = win::hwnd_of(&window);
            // W-9: cover the screen before any GPU work.
            let black = hwnd.map_or("no HWND", win::fill_black);
            log!(
                "window[{index}] created: id={:?} hwnd={hwnd:?} inner_size={:?} outer_position={:?} \
                 scale_factor={} fullscreen={:?} cursor hidden, black fill: {black}",
                window.id(),
                window.inner_size(),
                window.outer_position(),
                window.scale_factor(),
                window.fullscreen(),
            );
            self.windows.push(SaverWindow {
                window,
                target: None,
                label: format!("window[{index}]"),
                focused: false,
                frames: 0,
                frames_since_report: 0,
                last_report: Instant::now(),
                retry_at: None,
                black_fills_logged: 0,
            });
        }
    }

    /// Initializes wgpu for all windows. On failure the windows stay black (W-9).
    fn init_gpu(&mut self) {
        let started = Instant::now();
        let result = gpu::guarded("GPU initialization", || {
            let instance = gpu::create_instance(&self.options);
            let mut surfaces = Vec::new();
            for w in &self.windows {
                match gpu::create_surface(&instance, &w.window) {
                    Ok(surface) => surfaces.push(Some(surface)),
                    Err(e) => {
                        log!("{}: {e}", w.label);
                        surfaces.push(None);
                    }
                }
            }
            let first = surfaces
                .iter()
                .flatten()
                .next()
                .ok_or_else(|| "no window could create a surface".to_owned())?;
            let gpu = gpu::request_gpu(&instance, &self.options, Some(first))?;
            Ok((instance, gpu, surfaces))
        });

        match result {
            Ok((instance, gpu, surfaces)) => {
                for (w, surface) in self.windows.iter_mut().zip(surfaces) {
                    let Some(surface) = surface else { continue };
                    match gpu::Target::new(&gpu, surface, w.window.inner_size(), &w.label) {
                        Ok(target) => w.target = Some(target),
                        Err(e) => log!("{}: surface setup FAILED: {e}", w.label),
                    }
                }
                log!(
                    "GPU initialization finished in {} ms; {} of {} windows can render",
                    started.elapsed().as_millis(),
                    self.windows.iter().filter(|w| w.target.is_some()).count(),
                    self.windows.len(),
                );
                self.gpu = Some(gpu);
                self.instance = Some(instance);
            }
            Err(e) => {
                // W-9: never close the cover because of a GPU problem.
                log!(
                    "GPU initialization FAILED after {} ms: {e}; keeping the black windows and \
                     waiting for input",
                    started.elapsed().as_millis()
                );
            }
        }
    }

    fn log_cursor_moved(
        &mut self,
        index: usize,
        window_position: (f64, f64),
        screen: Option<(i32, i32)>,
    ) {
        if self.cursor_moves_logged < CURSOR_MOVES_LOGGED {
            self.cursor_moves_logged += 1;
            log!(
                "window[{index}] CursorMoved #{} window_position=({:.1}, {:.1}) screen_position={screen:?}",
                self.cursor_moves_logged,
                window_position.0,
                window_position.1,
            );
        } else if self.cursor_moves_logged == CURSOR_MOVES_LOGGED {
            self.cursor_moves_logged += 1;
            log!("further CursorMoved events are not logged (exit checks continue)");
        }
    }

    fn handle_cursor_moved(
        &mut self,
        event_loop: &ActiveEventLoop,
        index: usize,
        window_position: (f64, f64),
    ) {
        let screen = win::cursor_pos();
        self.log_cursor_moved(index, window_position, screen);
        let Some(now) = screen else { return };
        let Some(start) = self.start_cursor else {
            log!("start cursor position was unknown; using {now:?} as the reference");
            self.start_cursor = Some(now);
            return;
        };
        if policy::cursor_moved_enough(start, now, policy::CURSOR_EXIT_DISTANCE) {
            let what = format!(
                "cursor moved from {start:?} to {now:?} (at least {} px)",
                policy::CURSOR_EXIT_DISTANCE
            );
            self.input_decision(event_loop, &what);
        }
    }

    fn redraw(&mut self, index: usize) {
        let elapsed = self.started.elapsed();
        let w = &mut self.windows[index];
        let (Some(target), Some(gpu)) = (w.target.as_mut(), self.gpu.as_ref()) else {
            // No GPU for this window: keep it opaque black. No further redraw
            // is requested, so this does not spin.
            let outcome = win::hwnd_of(&w.window).map_or("no HWND", win::fill_black);
            if w.black_fills_logged < 3 {
                w.black_fills_logged += 1;
                log!("{}: redraw without GPU, GDI {outcome}", w.label);
            }
            return;
        };

        match target.draw(gpu, elapsed) {
            Frame::Presented => {
                w.frames += 1;
                w.frames_since_report += 1;
                if w.frames <= 3 {
                    log!("{}: frame {} presented", w.label, w.frames);
                }
                let since = w.last_report.elapsed();
                if since >= FRAME_REPORT_INTERVAL {
                    log!(
                        "{}: {} frames in {:.1} s ({:.1} fps), {} total",
                        w.label,
                        w.frames_since_report,
                        since.as_secs_f64(),
                        w.frames_since_report as f64 / since.as_secs_f64(),
                        w.frames,
                    );
                    w.frames_since_report = 0;
                    w.last_report = Instant::now();
                }
                // Continuous rendering; the Fifo present provides the vsync pacing.
                w.window.request_redraw();
            }
            Frame::Skipped => w.retry_at = Some(Instant::now() + RETRY_DELAY),
        }
    }
}

impl ApplicationHandler for SaverApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.created {
            log!("resumed again: ignored");
            return;
        }
        self.created = true;
        let begin = Instant::now();
        self.start_cursor = win::cursor_pos();
        log!(
            "resumed: cursor position at startup (screen) = {:?}",
            self.start_cursor
        );

        self.create_windows(event_loop);
        if self.windows.is_empty() {
            log!("no window could be created; exiting");
            self.exit_code = EXIT_NO_WINDOW;
            self.request_exit(event_loop, "no window");
            return;
        }
        self.init_gpu();

        // Start the grace period only now: events that queued up while the
        // windows and the GPU were being created are delivered inside it.
        self.grace_until = Some(Instant::now() + policy::GRACE_PERIOD);
        log!(
            "startup took {} ms; grace period of {} ms starts now",
            begin.elapsed().as_millis(),
            policy::GRACE_PERIOD.as_millis()
        );
        for w in &self.windows {
            w.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(index) = self.index_of(id) else {
            log!("event for an unknown window {id:?}: {event:?}");
            return;
        };
        match event {
            WindowEvent::Focused(focused) => {
                self.windows[index].focused = focused;
                let states: Vec<bool> = self.windows.iter().map(|w| w.focused).collect();
                log!(
                    "window[{index}] Focused({focused}); any app window focused: {}; per-window: {states:?} \
                     (logged only, never ends the screensaver)",
                    states.iter().any(|&f| f),
                );
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                log!(
                    "window[{index}] KeyboardInput state={:?} is_synthetic={is_synthetic} repeat={} \
                     logical_key={:?} physical_key={:?}",
                    event.state,
                    event.repeat,
                    event.logical_key,
                    event.physical_key,
                );
                if event.state == ElementState::Pressed && !is_synthetic {
                    let what = format!("key press {:?}", event.logical_key);
                    self.input_decision(event_loop, &what);
                } else {
                    log!("decision: key event is a release or synthetic, not an exit trigger");
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                log!("window[{index}] MouseInput state={state:?} button={button:?}");
                // Only a press ends the screensaver: a release can belong to the
                // click that started it.
                if state == ElementState::Pressed {
                    self.input_decision(event_loop, &format!("mouse button {button:?}"));
                }
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                log!("window[{index}] MouseWheel delta={delta:?} phase={phase:?}");
                self.input_decision(event_loop, "mouse wheel");
            }
            WindowEvent::Touch(touch) => {
                log!("window[{index}] Touch {touch:?}");
                self.input_decision(event_loop, "touch");
            }
            WindowEvent::CloseRequested => {
                log!("window[{index}] CloseRequested");
                self.input_decision(event_loop, "close requested");
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                log!("window[{index}] ScaleFactorChanged scale_factor={scale_factor}");
            }
            WindowEvent::Resized(size) => {
                log!("window[{index}] Resized {size:?}");
                if let (Some(target), Some(gpu)) =
                    (self.windows[index].target.as_mut(), self.gpu.as_ref())
                {
                    target.resize(gpu, size);
                }
            }
            WindowEvent::Moved(position) => {
                log!("window[{index}] Moved {position:?}");
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_cursor_moved(event_loop, index, (position.x, position.y));
            }
            WindowEvent::CursorEntered { .. } => log!("window[{index}] CursorEntered"),
            WindowEvent::CursorLeft { .. } => log!("window[{index}] CursorLeft"),
            WindowEvent::Occluded(occluded) => log!("window[{index}] Occluded({occluded})"),
            WindowEvent::Destroyed => log!("window[{index}] Destroyed"),
            WindowEvent::RedrawRequested => self.redraw(index),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Retry skipped frames after a short delay instead of spinning.
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        for w in &mut self.windows {
            if let Some(at) = w.retry_at {
                if at <= now {
                    w.retry_at = None;
                    w.window.request_redraw();
                } else {
                    next = Some(next.map_or(at, |n| n.min(at)));
                }
            }
        }
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        log!("exiting: dropping surfaces and windows");
        self.windows.clear();
        self.gpu = None;
        self.instance = None;
    }
}
