//! `--self-test` mode (spike S6 and the CI smoke-test feasibility check).
//!
//! Opens a normal 320x240 window, requests the software adapter (WARP on
//! DX12), renders 30 frames and exits. Exit codes: 0 = OK, 1 = failure,
//! 3 = timeout. The result is also written to the log as `SELF-TEST OK` or
//! `SELF-TEST FAILED: ...`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::gpu::{self, Frame};
use crate::log::log;
use crate::win;

const FRAMES_REQUIRED: u32 = 30;
const TIMEOUT: Duration = Duration::from_secs(20);

const EXIT_OK: i32 = 0;
const EXIT_FAILED: i32 = 1;
const EXIT_TIMEOUT: i32 = 3;

struct SelfTestApp {
    started: Instant,
    window: Option<Arc<Window>>,
    target: Option<gpu::Target>,
    gpu: Option<gpu::Gpu>,
    instance: Option<wgpu::Instance>,
    frames: u32,
    /// `Some` once the test has reached a verdict.
    outcome: Option<Result<(), String>>,
}

/// Runs the self test and returns the process exit code.
pub fn run() -> i32 {
    log!(
        "SELF-TEST start: {FRAMES_REQUIRED} frames, software adapter, DX12, timeout {} s",
        TIMEOUT.as_secs()
    );

    // The watchdog is a thread because a hung driver call would also block the
    // event loop, so a timer inside the loop could never fire.
    let watchdog = std::thread::Builder::new()
        .name("self-test-watchdog".to_owned())
        .spawn(|| {
            std::thread::sleep(TIMEOUT);
            log!(
                "SELF-TEST FAILED: timeout, {FRAMES_REQUIRED} frames were not rendered within {} s",
                TIMEOUT.as_secs()
            );
            std::process::exit(EXIT_TIMEOUT);
        });
    if let Err(e) = watchdog {
        log!("SELF-TEST: could not start the watchdog thread: {e}");
    }

    match std::panic::catch_unwind(run_inner) {
        Ok(Ok(())) => {
            log!("SELF-TEST OK");
            EXIT_OK
        }
        Ok(Err(e)) => {
            log!("SELF-TEST FAILED: {e}");
            EXIT_FAILED
        }
        Err(_) => {
            log!("SELF-TEST FAILED: panic (see PANIC line above)");
            EXIT_FAILED
        }
    }
}

fn run_inner() -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("EventLoop::new failed: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = SelfTestApp {
        started: Instant::now(),
        window: None,
        target: None,
        gpu: None,
        instance: None,
        frames: 0,
        outcome: None,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("event loop error: {e}"))?;
    app.outcome.take().unwrap_or_else(|| {
        Err(format!(
            "event loop ended after {} of {FRAMES_REQUIRED} frames without a verdict",
            app.frames
        ))
    })
}

impl SelfTestApp {
    fn finish(&mut self, event_loop: &ActiveEventLoop, outcome: Result<(), String>) {
        if self.outcome.is_none() {
            self.outcome = Some(outcome);
        }
        event_loop.exit();
    }

    fn setup(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attributes = Window::default_attributes()
            .with_title("saver-spike self-test")
            .with_inner_size(LogicalSize::new(320.0, 240.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| format!("create_window failed: {e}"))?,
        );
        log!(
            "SELF-TEST window created: hwnd={:?} inner_size={:?} scale_factor={}",
            win::hwnd_of(&window),
            window.inner_size(),
            window.scale_factor(),
        );

        // Fixed options: the software adapter on DX12, whatever the flag files say.
        let options = gpu::Options {
            force_fallback: true,
            vulkan: false,
        };
        let (instance, gpu, target) = gpu::guarded("GPU initialization", || {
            let instance = gpu::create_instance(&options);
            let surface = gpu::create_surface(&instance, &window)?;
            let gpu = gpu::request_gpu(&instance, &options, Some(&surface))?;
            let target = gpu::Target::new(&gpu, surface, window.inner_size(), "self-test")?;
            Ok((instance, gpu, target))
        })?;
        self.window = Some(window);
        self.instance = Some(instance);
        self.gpu = Some(gpu);
        self.target = Some(target);
        Ok(())
    }
}

impl ApplicationHandler for SelfTestApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() || self.outcome.is_some() {
            return;
        }
        match self.setup(event_loop) {
            Ok(()) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Err(e) => self.finish(event_loop, Err(e)),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.finish(
                    event_loop,
                    Err(format!(
                        "window closed after {} of {FRAMES_REQUIRED} frames",
                        self.frames
                    )),
                );
            }
            WindowEvent::Resized(size) => {
                if let (Some(target), Some(gpu)) = (self.target.as_mut(), self.gpu.as_ref()) {
                    target.resize(gpu, size);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.outcome.is_some() {
                    return;
                }
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
                        if self.frames == 1 || self.frames == FRAMES_REQUIRED {
                            log!("SELF-TEST frame {} presented", self.frames);
                        }
                        if self.frames >= FRAMES_REQUIRED {
                            self.finish(event_loop, Ok(()));
                        } else {
                            window.request_redraw();
                        }
                    }
                    Frame::Skipped => {
                        // Do not spin while the surface is unavailable.
                        std::thread::sleep(Duration::from_millis(10));
                        window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.target = None;
        self.gpu = None;
        self.instance = None;
        self.window = None;
    }
}
