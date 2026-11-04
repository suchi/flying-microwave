//! `/s`: the full-screen screensaver (Phase 1, task 6).
//!
//! One borderless, topmost window per monitor, in a single event loop. Each
//! window is painted black with GDI first (W-9), then wgpu draws the flyers
//! of its own simulation. Input ends the screensaver as decided by
//! [`flying_microwave_core::exit_policy`].
//!
//! The settings (plan 0004, F-06) are read on another thread while the
//! windows are created and painted black, so reading them never delays
//! covering the screen. They are waited for only before the animation starts.
//!
//! While it runs, the screensaver reads the monitor list again every
//! [`MONITOR_CHECK_INTERVAL`] and makes the windows follow it (plan 0004,
//! F-07): a window for each added monitor, moved or resized windows for
//! changed monitors, and no window for a removed monitor.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flying_microwave_core::config::{Config, Monitors};
use flying_microwave_core::exit_policy::{self, Decision, Event, ExitPolicy};
use flying_microwave_core::monitors::{self, Area, Change};
use flying_microwave_core::pacing::{FramePacer, Pace};
use flying_microwave_core::sim::{SimParams, World};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowId, WindowLevel};

use crate::log::log;
use crate::render::{self, Gpu, Target};
use crate::storage::Startup;
use crate::win;

/// Retry delay after a skipped frame (occluded surface and so on).
const RETRY_DELAY: Duration = Duration::from_millis(50);
/// If GPU initialization has not finished after this long, the watchdog ends
/// the process, so the black windows cannot cover the screen forever.
const GPU_INIT_TIMEOUT: Duration = Duration::from_secs(15);
/// How long the animation waits for the settings once the screen is covered.
/// Later, the defaults are used (plan 0001, N-01: first frame within 1 s).
const CONFIG_WAIT: Duration = Duration::from_millis(500);
/// How often the monitor list is read again while running (plan 0004, F-07).
///
/// Why polling: winit 0.30.13 passes no event for WM_DISPLAYCHANGE (its
/// platform_impl/windows/event_loop.rs turns only WM_DPICHANGED into an
/// event), and subclassing the windows to receive it was not chosen (plan
/// 0004, D-12). Why one second: a monitor that is plugged in stays uncovered
/// for at most about that long, and reading the list (EnumDisplayMonitors)
/// once a second costs next to nothing.
const MONITOR_CHECK_INTERVAL: Duration = Duration::from_secs(1);

const EXIT_OK: i32 = 0;
const EXIT_EVENT_LOOP_FAILED: i32 = 1;
const EXIT_NO_WINDOW: i32 = 5;
const EXIT_GPU_INIT_TIMEOUT: u32 = 6;

struct SaverWindow {
    window: Arc<Window>,
    /// Numbers the windows of this run in the log and for the exit policy.
    /// It never changes and is never reused.
    index: usize,
    /// The monitor this window covers and its area, if known.
    monitor: Option<(MonitorHandle, Area)>,
    /// Whether this window shows the animation; otherwise it stays black
    /// (`display.monitors = "primary"` on another monitor).
    animate: bool,
    /// `display.max_fps` for the monitor of this window.
    pacer: FramePacer,
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
    /// The settings, until they are taken once the windows exist.
    config_rx: Option<Receiver<Startup>>,
    params: SimParams,
    /// `display.max_fps`.
    max_fps: u32,
    /// `display.monitors = "primary"`.
    only_primary: bool,
    /// The monitor list of the last check (at start, the one the windows
    /// were created for).
    known_monitors: Vec<(MonitorHandle, Area)>,
    /// The `index` of the next window.
    next_index: usize,
    /// When the monitor list is read again; `None` until the windows and
    /// the GPU are ready.
    next_monitor_check: Option<Instant>,
    /// Whether the last check found no monitor at all (logged only once).
    monitors_empty: bool,
    /// The fixed start of the times passed to the frame pacers.
    epoch: Instant,
    /// Start of the grace period; `None` until the windows and the GPU are ready.
    grace_start: Option<Instant>,
    created: bool,
    exit_code: i32,
    exit_reason: Option<String>,
}

/// Runs the screensaver and returns the process exit code.
pub fn run() -> i32 {
    let config_rx = start_loading_config();
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
        // Replaced once the settings are read, before any input is handled.
        policy: ExitPolicy::new(
            exit_policy::Mode::Screensaver,
            ExitPolicy::DEFAULT_THRESHOLD_PX,
            ExitPolicy::DEFAULT_GRACE,
        ),
        config_rx: Some(config_rx),
        params: SimParams::default(),
        max_fps: 0,
        only_primary: false,
        known_monitors: Vec::new(),
        next_index: 0,
        next_monitor_check: None,
        monitors_empty: false,
        epoch: Instant::now(),
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

/// Starts reading the settings on another thread (A-5: `/s` never writes them).
fn start_loading_config() -> Receiver<Startup> {
    let (sender, receiver) = mpsc::channel();
    let thread_sender = sender.clone();
    let spawned = std::thread::Builder::new()
        .name("config-load".to_owned())
        .spawn(move || {
            let _ = thread_sender.send(Startup::load());
        });
    if let Err(e) = spawned {
        log!("config thread could not start: {e}; reading the settings here");
        let _ = sender.send(Startup::load());
    }
    receiver
}

/// Waits a little for the settings; the defaults when they are late.
fn receive_config(receiver: Option<Receiver<Startup>>) -> Config {
    let start = Instant::now();
    match receiver.map(|r| r.recv_timeout(CONFIG_WAIT)) {
        Some(Ok(startup)) => {
            log!(
                "{} (waited {} ms after the windows)",
                startup.summary(),
                start.elapsed().as_millis()
            );
            for note in &startup.loaded.notes {
                log!("config: {note}");
            }
            startup.loaded.config
        }
        _ => {
            log!(
                "config: not read within {} ms; using the defaults",
                CONFIG_WAIT.as_millis()
            );
            Config::default()
        }
    }
}

/// Width divided by height (1.0 for an empty size).
pub(crate) fn aspect_of(size: PhysicalSize<u32>) -> f32 {
    if size.height == 0 {
        1.0
    } else {
        size.width as f32 / size.height as f32
    }
}

/// The area of a monitor, in physical pixels.
fn area_of(monitor: &MonitorHandle) -> Area {
    let position = monitor.position();
    let size = monitor.size();
    Area {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

/// Creates a borderless, topmost window that covers the monitor area
/// `area` (maximized when it is unknown), hides the cursor and paints the
/// window black (W-9). `active` is whether showing the window activates it.
fn create_black_window(
    event_loop: &ActiveEventLoop,
    area: Option<Area>,
    index: usize,
    active: bool,
) -> Option<Arc<Window>> {
    let attributes = Window::default_attributes()
        .with_title("Flying Microwave")
        .with_decorations(false)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_active(active);
    // Why: not `with_fullscreen`. winit 0.30.13 then injects an
    // extended (right) Alt press with SendInput, which ended every
    // run on real hardware (docs/research/spike-results.md). A
    // borderless window that covers the monitor avoids it.
    let attributes = match area {
        Some(area) => attributes
            .with_position(PhysicalPosition::new(area.x, area.y))
            .with_inner_size(PhysicalSize::new(area.width, area.height)),
        None => attributes.with_maximized(true),
    };
    let window = match event_loop.create_window(attributes) {
        Ok(window) => Arc::new(window),
        Err(e) => {
            log!("window {index}: creation failed: {e}");
            return None;
        }
    };
    window.set_cursor_visible(false);
    if let Some(hwnd) = win::hwnd_of(&window) {
        win::fill_black(hwnd);
    }
    log!(
        "window {index}: inner {:?} outer position {:?} scale {}",
        window.inner_size(),
        window.outer_position(),
        window.scale_factor()
    );
    Some(window)
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
                "monitor {i}: position {:?} size {:?} scale {} refresh {:?} mHz",
                m.position(),
                m.size(),
                m.scale_factor(),
                m.refresh_rate_millihertz()
            );
        }
        let primary = event_loop.primary_monitor();
        self.known_monitors = monitors.iter().map(|m| (m.clone(), area_of(m))).collect();
        // Without monitor information, one maximized window.
        let targets: Vec<_> = if monitors.is_empty() {
            vec![None]
        } else {
            monitors.into_iter().map(Some).collect()
        };
        self.next_index = targets.len();
        // First cover every monitor in black (W-9); the settings come after.
        let mut created = Vec::new();
        for (index, monitor) in targets.into_iter().enumerate() {
            let monitor = monitor.map(|m| {
                let area = area_of(&m);
                (m, area)
            });
            let area = monitor.as_ref().map(|(_, area)| *area);
            if let Some(window) = create_black_window(event_loop, area, index, true) {
                created.push((index, window, monitor));
            }
        }

        let config = receive_config(self.config_rx.take());
        log!("settings: {config:?}");
        self.params = config.sim_params();
        self.max_fps = config.display.max_fps.value();
        self.only_primary = config.display.monitors == Monitors::Primary;
        self.policy = ExitPolicy::new(
            exit_policy::Mode::Screensaver,
            config.input.exit_sensitivity.threshold_px(),
            ExitPolicy::DEFAULT_GRACE,
        );
        // Which windows animate: all, or only the primary monitor's. If no
        // window is on the primary monitor, the first window animates, so
        // that the animation is always shown somewhere.
        let on_primary = |monitor: &Option<(MonitorHandle, Area)>| {
            monitor
                .as_ref()
                .is_some_and(|(m, _)| primary.as_ref() == Some(m))
        };
        let any_on_primary = created.iter().any(|(_, _, monitor)| on_primary(monitor));
        for (position, (index, window, monitor)) in created.into_iter().enumerate() {
            let animate = !self.only_primary
                || if any_on_primary {
                    on_primary(&monitor)
                } else {
                    position == 0
                };
            let w = self.saver_window(index, window, monitor, animate);
            self.windows.push(w);
        }
        // Keyboard input only reaches the foreground window (see win::set_foreground).
        if let Some(hwnd) = self.windows.first().and_then(|w| win::hwnd_of(&w.window)) {
            log!("SetForegroundWindow: {}", win::set_foreground(hwnd));
        }
    }

    /// The state of a new window, before it has a render target.
    fn saver_window(
        &self,
        index: usize,
        window: Arc<Window>,
        monitor: Option<(MonitorHandle, Area)>,
        animate: bool,
    ) -> SaverWindow {
        let refresh = monitor
            .as_ref()
            .and_then(|(m, _)| m.refresh_rate_millihertz());
        let pacer = FramePacer::new(self.max_fps, refresh);
        log!(
            "window {index}: animate {animate}, frame rate limited {}",
            pacer.limits()
        );
        let size = window.inner_size();
        SaverWindow {
            world: World::new(seed(index), aspect_of(size), self.params),
            aspect: aspect_of(size),
            window,
            index,
            monitor,
            animate,
            pacer,
            target: None,
            last_frame: None,
            presented: false,
            retry_at: None,
        }
    }

    /// Creates the render target of a window made while running. Without a
    /// GPU, or when that fails, the window stays black (W-9).
    fn attach_target(&mut self, position: usize) {
        let w = &self.windows[position];
        let index = w.index;
        let (Some(instance), Some(gpu)) = (self.instance.as_ref(), self.gpu.as_ref()) else {
            log!("window {index}: no GPU; it stays black");
            return;
        };
        // Why the watchdog: as at start, a hung driver call must not leave
        // the black windows on the screen for good.
        let _watchdog = render::Watchdog::start(GPU_INIT_TIMEOUT, EXIT_GPU_INIT_TIMEOUT);
        let sprite_size = w.world.instances().first().map_or(0.12, |s| s.size);
        log!("window {index}: setting up the surface");
        let result = render::guarded("surface setup", || {
            let surface = render::create_surface(instance, &w.window)?;
            Target::new(gpu, surface, w.window.inner_size(), sprite_size)
        });
        match result {
            Ok(target) => self.windows[position].target = Some(target),
            Err(e) => log!("window {index}: surface setup failed: {e}; it stays black"),
        }
    }

    /// F-07: reads the monitor list again and makes the windows follow it.
    fn check_monitors(&mut self, event_loop: &ActiveEventLoop) {
        let current: Vec<(MonitorHandle, Area)> = event_loop
            .available_monitors()
            .map(|m| {
                let area = area_of(&m);
                (m, area)
            })
            .collect();
        let Some(changes) = monitors::compare(&self.known_monitors, &current) else {
            if !self.monitors_empty {
                log!("monitor change: the monitor list is empty; keeping the windows");
                self.monitors_empty = true;
            }
            return;
        };
        self.monitors_empty = false;
        if changes.iter().any(|c| !matches!(c, Change::Kept { .. })) {
            log!("monitor change: {} monitors now", current.len());
            for (i, (m, area)) in current.iter().enumerate() {
                log!(
                    "monitor change: monitor {i}: {area:?} scale {} refresh {:?} mHz",
                    m.scale_factor(),
                    m.refresh_rate_millihertz()
                );
            }
        }
        // The window of each known monitor (none if it could not be made),
        // looked up before any window changes.
        let windows_of_known: Vec<Option<usize>> = self
            .known_monitors
            .iter()
            .map(|(known, _)| {
                self.windows
                    .iter()
                    .position(|w| w.monitor.as_ref().is_some_and(|(m, _)| m == known))
            })
            .collect();
        let primary = event_loop.primary_monitor();
        let mut closing = Vec::new();
        for change in changes {
            match change {
                Change::Kept { old, new } => {
                    if let Some(p) = windows_of_known[old] {
                        let w = &mut self.windows[p];
                        if w.monitor
                            .as_ref()
                            .is_some_and(|(m, _)| *m != current[new].0)
                        {
                            log!("window {}: its monitor has a new handle", w.index);
                        }
                        w.monitor = Some(current[new].clone());
                    }
                }
                Change::Moved { old, new } => {
                    if let Some(p) = windows_of_known[old] {
                        self.move_window(p, current[new].clone());
                    }
                }
                Change::Added { new } => {
                    self.add_window(event_loop, current[new].clone(), primary.as_ref());
                }
                Change::Removed { old } => closing.extend(windows_of_known[old]),
            }
        }
        self.close_windows(event_loop, closing);
        self.known_monitors = current;
    }

    /// Fits a window to the new position and size of its monitor.
    fn move_window(&mut self, position: usize, monitor: (MonitorHandle, Area)) {
        let area = monitor.1;
        let refresh = monitor.0.refresh_rate_millihertz();
        let w = &mut self.windows[position];
        log!("window {}: moving to {area:?}", w.index);
        // Why before moving: moving onto a monitor with another scale factor
        // raises ScaleFactorChanged, which asks for the size of `w.monitor`.
        w.monitor = Some(monitor);
        w.pacer = FramePacer::new(self.max_fps, refresh);
        w.window
            .set_outer_position(PhysicalPosition::new(area.x, area.y));
        // The render target and the world follow in the Resized event.
        let resized = w
            .window
            .request_inner_size(PhysicalSize::new(area.width, area.height));
        if let Some(size) = resized {
            self.fit_to_size(position, size);
        }
    }

    /// Covers a monitor that was added while running, as at start.
    fn add_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        monitor: (MonitorHandle, Area),
        primary: Option<&MonitorHandle>,
    ) {
        let index = self.next_index;
        self.next_index += 1;
        // Why not activated: activating it would take the keyboard focus from
        // the foreground window. Were it activated anyway, the focus would
        // only move between our windows, which does not end the screensaver
        // (exit_policy: the loss and the gain arrive in one batch).
        let Some(window) = create_black_window(event_loop, Some(monitor.1), index, false) else {
            return;
        };
        log!("window {index}: created for a new monitor");
        let animate = !self.only_primary || primary == Some(&monitor.0);
        let w = self.saver_window(index, window, Some(monitor), animate);
        self.windows.push(w);
        let position = self.windows.len() - 1;
        if animate {
            self.attach_target(position);
        }
        self.windows[position].window.request_redraw();
    }

    /// Closes the windows at `positions`, whose monitors are gone.
    fn close_windows(&mut self, event_loop: &ActiveEventLoop, mut positions: Vec<usize>) {
        if positions.is_empty() {
            return;
        }
        positions.sort_unstable();
        positions.dedup();
        if positions.len() >= self.windows.len() {
            log!("monitor change: no window would remain; keeping the windows");
            return;
        }
        // Why: keyboard input only reaches the foreground window. When it is
        // closed, Windows activates some other window, maybe not ours, and
        // the focus loss would end the screensaver. So the foreground moves
        // to a remaining window first.
        let foreground = win::foreground_window();
        let closes_foreground = positions
            .iter()
            .any(|&p| win::hwnd_of(&self.windows[p].window) == Some(foreground));
        if closes_foreground
            && let Some(hwnd) = (0..self.windows.len())
                .filter(|p| !positions.contains(p))
                .find_map(|p| win::hwnd_of(&self.windows[p].window))
        {
            log!("SetForegroundWindow: {}", win::set_foreground(hwnd));
        }
        for &p in positions.iter().rev() {
            let w = self.windows.remove(p);
            log!("window {}: closed, its monitor is gone", w.index);
            // A closed window is no longer in `windows`, so its own focus and
            // Destroyed events are ignored (window_event). Tell the exit
            // policy here that it has no focus any more.
            self.decide(
                event_loop,
                Event::FocusChanged {
                    window: exit_policy::WindowId(w.index as u64),
                    focused: false,
                },
            );
        }
    }

    /// Fits the render target and the world of a window to its new size.
    fn fit_to_size(&mut self, position: usize, size: PhysicalSize<u32>) {
        let w = &mut self.windows[position];
        if let (Some(target), Some(gpu)) = (w.target.as_mut(), self.gpu.as_ref()) {
            target.resize(gpu, size);
        }
        // The aspect only matters where flyers re-enter, and the size
        // changes almost only right after start, so a new world is fine.
        let aspect = aspect_of(size);
        if size.height > 0 && (aspect - w.aspect).abs() > 1e-3 {
            w.world = World::new(seed(w.index), aspect, self.params);
            w.aspect = aspect;
            w.last_frame = None;
        }
    }

    /// Initializes wgpu for every window. On failure the windows stay black (W-9).
    fn init_gpu(&mut self) {
        let _watchdog = render::Watchdog::start(GPU_INIT_TIMEOUT, EXIT_GPU_INIT_TIMEOUT);
        let result = render::guarded("GPU initialization", || {
            let instance = render::create_instance();
            // A window that does not animate needs no surface: it stays black.
            let surfaces: Vec<_> = self
                .windows
                .iter()
                .map(|w| {
                    w.animate
                        .then(|| match render::create_surface(&instance, &w.window) {
                            Ok(surface) => Some(surface),
                            Err(e) => {
                                log!("window {}: {e}; it stays black", w.index);
                                None
                            }
                        })
                        .flatten()
                })
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
                    // Why: the lines that Target::new writes do not name the
                    // window, and with several monitors the log must tell
                    // which window they belong to (Issue #50).
                    log!("window {}: setting up the surface", w.index);
                    match Target::new(&gpu, surface, w.window.inner_size(), sprite_size) {
                        Ok(target) => w.target = Some(target),
                        Err(e) => log!("window {}: surface setup failed: {e}", w.index),
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
            // No GPU for this window, or it does not animate: keep it black,
            // no further redraws.
            if let Some(hwnd) = win::hwnd_of(&w.window) {
                win::fill_black(hwnd);
            }
            return;
        };
        let now = Instant::now();
        // display.max_fps: skip this frame and draw at the given time.
        if let Pace::WaitUntil(at) = w.pacer.poll(now - self.epoch) {
            w.retry_at = Some(self.epoch + at);
            return;
        }
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
        self.next_monitor_check = Some(Instant::now() + MONITOR_CHECK_INTERVAL);
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
            // Why `index` of the window, not its position in `windows`:
            // windows are closed while running (F-07), which moves the
            // positions, and the exit policy must keep telling them apart.
            WindowEvent::Focused(focused) => Some(Event::FocusChanged {
                window: exit_policy::WindowId(self.windows[index].index as u64),
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
                if let Some((_, area)) = w.monitor
                    && let Err(e) = inner_size_writer
                        .request_inner_size(PhysicalSize::new(area.width, area.height))
                {
                    log!("window {}: keeping the monitor size failed: {e}", w.index);
                }
                None
            }
            WindowEvent::Resized(size) => {
                log!("window {}: resized to {size:?}", self.windows[index].index);
                self.fit_to_size(index, size);
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
        if self.exit_reason.is_none()
            && let Some(at) = self.next_monitor_check
            && at <= Instant::now()
        {
            self.check_monitors(event_loop);
            self.next_monitor_check = Some(Instant::now() + MONITOR_CHECK_INTERVAL);
        }
        let now = Instant::now();
        // Wake when the grace period ends, so a focus loss during it is
        // checked even if no further event arrives (ExitPolicy::handle).
        let mut next = self
            .grace_start
            .map(|t| t + ExitPolicy::DEFAULT_GRACE)
            .filter(|&end| end > now);
        // Wake for the next monitor check, even when no window draws.
        if let Some(at) = self.next_monitor_check {
            next = Some(next.map_or(at, |n| n.min(at)));
        }
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
