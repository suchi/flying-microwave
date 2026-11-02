//! `/s`: the full-screen screensaver (Phase 1, task 6).
//!
//! One borderless, topmost window per monitor, in a single event loop. Each
//! window is painted black with GDI first (W-9), then wgpu draws the flyers
//! of its own simulation. Input ends the screensaver as decided by
//! [`flying_microwave_core::exit_policy`].

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flying_microwave_core::exit_policy::{self, Decision, Event, ExitPolicy};
use flying_microwave_core::sim::{SimParams, World};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId, WindowLevel};

use crate::log::log;
use crate::render::{self, Gpu, Target};
use crate::win;

/// Retry delay after a skipped frame (occluded surface and so on).
const RETRY_DELAY: Duration = Duration::from_millis(50);
/// If GPU initialization has not finished after this long, the watchdog ends
/// the process, so the black windows cannot cover the screen forever.
const GPU_INIT_TIMEOUT: Duration = Duration::from_secs(15);

const EXIT_OK: i32 = 0;
const EXIT_EVENT_LOOP_FAILED: i32 = 1;
const EXIT_NO_WINDOW: i32 = 5;
const EXIT_GPU_INIT_TIMEOUT: u32 = 6;

struct SaverWindow {
    window: Arc<Window>,
    index: usize,
    /// Physical size of the monitor this window covers, if known.
    monitor_size: Option<PhysicalSize<u32>>,
    /// `None` when the GPU failed: the window then stays black.
    target: Option<Target>,
    world: World,
    /// The aspect `world` was created with.
    aspect: f32,
    last_frame: Option<Instant>,
    retry_at: Option<Instant>,
    /// Whether a frame has been presented, for the N-01 log line.
    presented: bool,
}

struct SaverApp {
    windows: Vec<SaverWindow>,
    // Dropped after the targets created from them (struct field order).
    gpu: Option<Gpu>,
    instance: Option<wgpu::Instance>,
    policy: ExitPolicy,
    /// Start of the grace period; `None` until the windows and the GPU are ready.
    grace_start: Option<Instant>,
    created: bool,
    exit_code: i32,
    exit_reason: Option<String>,
}

/// Runs the screensaver and returns the process exit code.
pub fn run() -> i32 {
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            log!("EventLoop::new failed: {e}");
            return EXIT_EVENT_LOOP_FAILED;
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = SaverApp {
        windows: Vec::new(),
        gpu: None,
        instance: None,
        policy: ExitPolicy::new(
            exit_policy::Mode::Screensaver,
            ExitPolicy::DEFAULT_THRESHOLD_PX,
            ExitPolicy::DEFAULT_GRACE,
        ),
        grace_start: None,
        created: false,
        exit_code: EXIT_OK,
        exit_reason: None,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        log!("event loop error: {e}");
        return EXIT_EVENT_LOOP_FAILED;
    }
    log!(
        "screensaver ended: {}",
        app.exit_reason.as_deref().unwrap_or("no reason")
    );
    app.exit_code
}

/// Width divided by height (1.0 for an empty size).
pub(crate) fn aspect_of(size: PhysicalSize<u32>) -> f32 {
    if size.height == 0 {
        1.0
    } else {
        size.width as f32 / size.height as f32
    }
}

/// A seed that differs between runs and between monitors.
pub(crate) fn seed(index: usize) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    nanos ^ (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

impl SaverApp {
    fn index_of(&self, id: WindowId) -> Option<usize> {
        self.windows.iter().position(|w| w.window.id() == id)
    }

    /// Time since the grace period started (zero before it starts).
    fn now(&self) -> Duration {
        self.grace_start.map_or(Duration::ZERO, |t| t.elapsed())
    }

    /// Passes an event to the exit policy and stops the loop when it says so.
    fn decide(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        if self.policy.handle(event, self.now()) == Decision::Exit && self.exit_reason.is_none() {
            log!("exit: {event:?}");
            self.exit_reason = Some(format!("{event:?}"));
            // Why: winit destroys the windows only after the loop has ended,
            // and the GPU is released before that, so without hiding them
            // here the screen would stay until the process ends (N-02 counts
            // until the screen is gone; plan 0003, D-6). set_visible runs
            // synchronously on the event loop thread (winit 0.30.13).
            for w in &self.windows {
                w.window.set_visible(false);
            }
            event_loop.exit();
        }
    }

    fn create_windows(&mut self, event_loop: &ActiveEventLoop) {
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        log!("monitors: {}", monitors.len());
        for (i, m) in monitors.iter().enumerate() {
            log!(
                "monitor {i}: position {:?} size {:?} scale {}",
                m.position(),
                m.size(),
                m.scale_factor()
            );
        }
        // Without monitor information, one maximized window.
        let targets: Vec<_> = if monitors.is_empty() {
            vec![None]
        } else {
            monitors.into_iter().map(Some).collect()
        };
        for (index, monitor) in targets.into_iter().enumerate() {
            let attributes = Window::default_attributes()
                .with_title("Flying Microwave")
                .with_decorations(false)
                .with_window_level(WindowLevel::AlwaysOnTop);
            // Why: not `with_fullscreen`. winit 0.30.13 then injects an
            // extended (right) Alt press with SendInput, which ended every
            // run on real hardware (docs/research/spike-results.md). A
            // borderless window that covers the monitor avoids it.
            let attributes = match &monitor {
                Some(monitor) => {
                    let position = monitor.position();
                    let size = monitor.size();
                    attributes
                        .with_position(PhysicalPosition::new(position.x, position.y))
                        .with_inner_size(PhysicalSize::new(size.width, size.height))
                }
                None => attributes.with_maximized(true),
            };
            let window = match event_loop.create_window(attributes) {
                Ok(window) => Arc::new(window),
                Err(e) => {
                    log!("window {index}: creation failed: {e}");
                    continue;
                }
            };
            window.set_cursor_visible(false);
            if let Some(hwnd) = win::hwnd_of(&window) {
                win::fill_black(hwnd);
            }
            let size = window.inner_size();
            log!(
                "window {index}: inner {size:?} outer position {:?} scale {}",
                window.outer_position(),
                window.scale_factor()
            );
            self.windows.push(SaverWindow {
                world: World::new(seed(index), aspect_of(size), SimParams::default()),
                aspect: aspect_of(size),
                window,
                index,
                monitor_size: monitor.as_ref().map(|m| m.size()),
                target: None,
                last_frame: None,
                presented: false,
                retry_at: None,
            });
        }
        // Keyboard input only reaches the foreground window (see win::set_foreground).
        if let Some(hwnd) = self.windows.first().and_then(|w| win::hwnd_of(&w.window)) {
            log!("SetForegroundWindow: {}", win::set_foreground(hwnd));
        }
    }

    /// Initializes wgpu for every window. On failure the windows stay black (W-9).
    fn init_gpu(&mut self) {
        let _watchdog = render::Watchdog::start(GPU_INIT_TIMEOUT, EXIT_GPU_INIT_TIMEOUT);
        let result = render::guarded("GPU initialization", || {
            let instance = render::create_instance();
            let surfaces: Vec<_> = self
                .windows
                .iter()
                .map(|w| render::create_surface(&instance, &w.window).ok())
                .collect();
            let first = surfaces
                .iter()
                .flatten()
                .next()
                .ok_or("no window could create a surface")?;
            let gpu = render::request_gpu(&instance, first)?;
            Ok((instance, gpu, surfaces))
        });
        match result {
            Ok((instance, gpu, surfaces)) => {
                for (w, surface) in self.windows.iter_mut().zip(surfaces) {
                    let Some(surface) = surface else { continue };
                    let sprite_size = w.world.instances().first().map_or(0.12, |s| s.size);
                    match Target::new(&gpu, surface, w.window.inner_size(), sprite_size) {
                        Ok(target) => w.target = Some(target),
                        Err(e) => log!("surface setup failed: {e}"),
                    }
                }
                self.gpu = Some(gpu);
                self.instance = Some(instance);
            }
            Err(e) => log!("GPU initialization failed: {e}; keeping the black windows"),
        }
    }

    fn redraw(&mut self, index: usize) {
        let w = &mut self.windows[index];
        let (Some(target), Some(gpu)) = (w.target.as_mut(), self.gpu.as_ref()) else {
            // No GPU for this window: keep it black, no further redraws.
            if let Some(hwnd) = win::hwnd_of(&w.window) {
                win::fill_black(hwnd);
            }
            return;
        };
        let now = Instant::now();
        if let Some(last) = w.last_frame {
            w.world.advance(now - last);
        }
        w.last_frame = Some(now);
        if target.draw(gpu, &w.world.instances()) {
            // N-01 (first animation frame within 1 s) is measured from this line.
            if !w.presented {
                w.presented = true;
                log!("window {}: first frame presented", w.index);
            }
            // Continuous rendering; the Fifo present paces it to the display.
            w.window.request_redraw();
        } else {
            w.retry_at = Some(now + RETRY_DELAY);
        }
    }
}

impl ApplicationHandler for SaverApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.created {
            return;
        }
        self.created = true;
        self.create_windows(event_loop);
        if self.windows.is_empty() {
            log!("no window could be created");
            self.exit_code = EXIT_NO_WINDOW;
            self.exit_reason = Some("no window".to_owned());
            event_loop.exit();
            return;
        }
        self.init_gpu();
        // Start the grace period only now: events queued while the windows
        // and the GPU were being set up are delivered inside it.
        self.grace_start = Some(Instant::now());
        for w in &self.windows {
            w.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let decision_event = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => Some(match event.state {
                ElementState::Pressed => Event::KeyPressed {
                    synthetic: is_synthetic,
                },
                ElementState::Released => Event::KeyReleased {
                    synthetic: is_synthetic,
                },
            }),
            // Only a press counts: a release can belong to the click that started us.
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => Some(Event::MouseButtonPressed),
            WindowEvent::MouseWheel { .. } => Some(Event::MouseWheel),
            WindowEvent::Touch(_) => Some(Event::Touch),
            // Screen coordinates, so movement across monitors is measured correctly.
            WindowEvent::CursorMoved { .. } => {
                win::cursor_pos().map(|(x, y)| Event::CursorMoved { x, y })
            }
            WindowEvent::Focused(focused) => Some(Event::FocusChanged {
                window: exit_policy::WindowId(index as u64),
                focused,
            }),
            WindowEvent::CloseRequested => Some(Event::CloseRequested),
            WindowEvent::Destroyed => Some(Event::WindowDestroyed),
            // Why: when the window moves to a monitor with another scale
            // factor, winit keeps its logical size, so it would no longer
            // cover the monitor. Ask for the monitor's physical size instead
            // (winit 0.30.13, WM_DPICHANGED in platform_impl/windows/event_loop.rs).
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                mut inner_size_writer,
            } => {
                let w = &self.windows[index];
                log!("window {}: scale factor changed to {scale_factor}", w.index);
                if let Some(size) = w.monitor_size
                    && let Err(e) = inner_size_writer.request_inner_size(size)
                {
                    log!("window {}: keeping the monitor size failed: {e}", w.index);
                }
                None
            }
            WindowEvent::Resized(size) => {
                let w = &mut self.windows[index];
                log!("window {}: resized to {size:?}", w.index);
                if let (Some(target), Some(gpu)) = (w.target.as_mut(), self.gpu.as_ref()) {
                    target.resize(gpu, size);
                }
                // The aspect only matters where flyers re-enter, and the size
                // changes almost only right after start, so a new world is fine.
                let aspect = aspect_of(size);
                if size.height > 0 && (aspect - w.aspect).abs() > 1e-3 {
                    w.world = World::new(seed(w.index), aspect, SimParams::default());
                    w.aspect = aspect;
                    w.last_frame = None;
                }
                None
            }
            WindowEvent::RedrawRequested => {
                self.redraw(index);
                None
            }
            _ => None,
        };
        if let Some(event) = decision_event {
            self.decide(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.decide(event_loop, Event::EventsProcessed);
        let now = Instant::now();
        // Wake when the grace period ends, so a focus loss during it is
        // checked even if no further event arrives (ExitPolicy::handle).
        let mut next = self
            .grace_start
            .map(|t| t + ExitPolicy::DEFAULT_GRACE)
            .filter(|&end| end > now);
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
        // Surfaces before the device and the instance.
        self.windows.clear();
        self.gpu = None;
        self.instance = None;
    }
}
