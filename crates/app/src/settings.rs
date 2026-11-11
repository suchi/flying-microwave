//! The settings window (plan 0004, F-03 to F-05, F-08 and F-09).
//!
//! An eframe window with the Flight, Display and About tabs and the
//! Defaults, OK and Cancel buttons. OK writes the settings file
//! ([`crate::storage`]); `/s` and `/p` only read it.
//!
//! eframe has no API for a native owner window, so the owner from `/c` is
//! attached right after the window exists, as in spike S2: the owner becomes
//! the window's owner and is disabled (modal behavior). On every way out
//! (OK, Cancel, closing the window, the end of the event loop, a panic) the
//! owner is enabled again and brought to the foreground, before our window
//! goes away, so that Windows does not activate some unrelated window.
//!
//! Hardening: SEC-1 is applied in `main` for every mode; SEC-2 (DX12 only,
//! the Fxc compiler, no `WGPU_*` environment variables) in
//! [`wgpu_configuration`].

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Button, Color32, CornerRadius, FontFamily, FontId, Frame, Label, Layout, Margin,
    RichText, Slider, Stroke, TextStyle, Theme, Ui, vec2,
};
use flying_microwave_core::config::{
    self, COUNT_RANGE, Config, ExitSensitivity, MaxFps, Monitors, SCALE_RANGE, SPEED_RANGE,
};
use flying_microwave_core::flight::Style;
use windows_sys::Win32::Foundation::HWND;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::cli::Owner;
use crate::gpu_limit::{self, AfterRun, Disarm, Limit, SettingsTimeout};
use crate::licenses;
use crate::log::log;
use crate::storage::{self, Location, Startup};
use crate::win;

/// Title of the settings window (the smoke test looks for it).
pub const TITLE: &str = "Flying Microwave Settings";
const PROJECT_URL: &str = "https://github.com/suchi/flying-microwave";

const EXIT_OK: i32 = 0;
/// The window could not be opened (E-4), also when the GPU initialization
/// limit is reached (plan 0004, D-17).
const EXIT_WINDOW_FAILED: i32 = 7;
/// The same limit as `/s` and `/p` (plan 0001, D-27; plan 0004, D-17),
/// counted from before `eframe::run_native`.
const GPU_INIT_TIMEOUT: Duration = Duration::from_secs(15);

// Colors of the approved mockup (plan 0004, N-02).
const BG: Color32 = Color32::from_rgb(0x0f, 0x12, 0x1a);
const PANEL: Color32 = Color32::from_rgb(0x16, 0x1a, 0x24);
const LINE: Color32 = Color32::from_rgb(0x26, 0x2c, 0x3a);
const TEXT: Color32 = Color32::from_rgb(0xd6, 0xdb, 0xe6);
const DIM: Color32 = Color32::from_rgb(0x8b, 0x93, 0xa5);
const ACCENT: Color32 = Color32::from_rgb(0x5e, 0xe7, 0xdf);
const ACCENT_INK: Color32 = Color32::from_rgb(0x06, 0x23, 0x20);
const WIDGET: Color32 = Color32::from_rgb(0x22, 0x28, 0x38);
const WIDGET_HOVER: Color32 = Color32::from_rgb(0x2b, 0x32, 0x45);

/// Size of the window in logical points (it follows the display scale, N-01).
const WINDOW_SIZE: [f32; 2] = [520.0, 420.0];
const LABEL_WIDTH: f32 = 170.0;
const ROW_HEIGHT: f32 = 26.0;

/// Window handle of the owner (0: none, or already released).
static OWNER: AtomicUsize = AtomicUsize::new(0);
/// Whether this process disabled the owner and still has to enable it.
static OWNER_DISABLED: AtomicBool = AtomicBool::new(false);

/// Shows the settings window and returns the process exit code.
pub fn run(owner: Owner) -> i32 {
    // A crash must not leave the owner dialog disabled (F-03).
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log!("panic: {info}");
        release_owner("panic");
        previous(info);
    }));

    let startup = Startup::load();
    log!("{}", startup.summary());
    for note in &startup.loaded.notes {
        log!("config: {note}");
    }
    let location = startup.location.clone();
    let owner = resolve_owner(owner);
    // Read once, when the window opens, not on every visit of the About tab.
    let notices = match licenses::load() {
        Ok(text) => {
            log!(
                "settings: {} read ({} bytes)",
                licenses::FILE_NAME,
                text.len()
            );
            Some(text)
        }
        Err(reason) => {
            log!("settings: {} not shown: {reason}", licenses::FILE_NAME);
            None
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size(WINDOW_SIZE)
            .with_resizable(false)
            .with_minimize_button(false)
            .with_maximize_button(false)
            // Why: the default egui icon is not ours to ship.
            .with_icon(egui::IconData::default()),
        renderer: eframe::Renderer::Wgpu,
        centered: true,
        // Why: the system_fonts feature is off, so there is no fallback.
        system_font_fallback: false,
        wgpu_options: wgpu_configuration(),
        ..Default::default()
    };
    log!("settings: starting the window");
    let failure_text = open_failure_text(&location);
    let limit = start_limit(failure_text.clone());
    let creator_limit = limit.clone();
    let result = eframe::run_native(
        "flying-microwave-settings",
        options,
        Box::new(move |cc| {
            // Plan 0005, D-1 (1): once the limit has fired, the owner must
            // not be disabled any more; the timer thread ends the process.
            if creator_limit.as_ref().map(Limit::disarm) == Some(Disarm::TooLate) {
                return Err("the GPU initialization limit was reached".into());
            }
            Ok(Box::new(SettingsApp::new(cc, startup, owner, notices)))
        }),
    );
    if gpu_limit::after_run(limit.as_ref().map_or(Disarm::InTime, Limit::disarm))
        == AfterRun::WaitForExit
    {
        // Plan 0005, D-1 (3): returning would end the process and take the
        // message box of the timer thread with it.
        log!("settings: waiting for the GPU initialization limit to end the process");
        loop {
            std::thread::park();
        }
    }
    release_owner("the window has closed");
    match result {
        Ok(()) => {
            log!("settings: closed");
            EXIT_OK
        }
        Err(e) => {
            log!("settings: the window could not be opened: {e}");
            // Why: decided in ADR-0024 (E-4). The settings
            // mode is interactive, so a message box is fine here.
            win::error_box(&failure_text, "Flying Microwave");
            EXIT_WINDOW_FAILED
        }
    }
}

/// Starts the GPU initialization limit of the settings window. Without the
/// timer thread the window still opens, as before the limit existed.
fn start_limit(failure_text: String) -> Option<Limit> {
    let started = Limit::start(GPU_INIT_TIMEOUT, move || {
        gpu_limit::settings_timed_out(&mut TimeoutEffects { failure_text });
    });
    match started {
        Ok(limit) => Some(limit),
        Err(e) => {
            log!("watchdog thread could not start: {e}");
            None
        }
    }
}

/// The effects of a reached limit (plan 0004, D-17; plan 0005, D-1 (2)).
struct TimeoutEffects {
    failure_text: String,
}

impl SettingsTimeout for TimeoutEffects {
    fn log(&mut self) {
        // The smoke test reads this line (plan 0005, D-1 (4)).
        log!("GPU initialization timed out; the settings window could not be opened");
    }

    fn release_owner(&mut self) {
        release_owner("GPU initialization timed out");
    }

    fn show_message(&mut self) {
        win::error_box(&self.failure_text, "Flying Microwave");
    }

    fn exit(&mut self) {
        win::terminate_now(EXIT_WINDOW_FAILED as u32);
    }
}

/// The message when the window cannot be opened (E-4): where the file is,
/// so that it can still be edited with a text editor.
fn open_failure_text(location: &Result<Location, String>) -> String {
    let place = match location {
        Ok(location) => format!(
            "The settings are kept in this file, which can be edited with a text editor:\n{}",
            location.path.display()
        ),
        Err(reason) => format!("The location of the settings file is unknown: {reason}."),
    };
    format!(
        "The settings window could not be opened (the graphics could not be started).\n\n{place}"
    )
}

/// SEC-3: the owner window, checked to be a real window.
fn resolve_owner(owner: Owner) -> Option<HWND> {
    let candidate = match owner {
        Owner::None => return None,
        Owner::Foreground => win::foreground_window(),
        Owner::Window(handle) => win::hwnd_from_arg(handle),
    };
    let valid = !candidate.is_null() && win::is_window(candidate);
    log!(
        "settings: owner {owner:?} is {} (HWND {})",
        if valid {
            "a window"
        } else {
            "not a window; no owner"
        },
        candidate as usize
    );
    valid.then_some(candidate)
}

/// Makes `owner` the owner of the settings window and disables it.
fn attach_owner(cc: &eframe::CreationContext<'_>, owner: Option<HWND>) {
    let Some(owner) = owner else { return };
    // Check again: time has passed since the command line was read.
    if !win::is_window(owner) {
        log!("settings: the owner is gone; continuing without one");
        return;
    }
    let window = match cc.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get() as HWND,
        Ok(_) | Err(_) => {
            log!("settings: no Win32 window handle; continuing without an owner");
            return;
        }
    };
    if let Err(code) = win::set_owner(window, owner) {
        log!("settings: setting the owner failed: error {code}; continuing without one");
        return;
    }
    OWNER.store(owner as usize, Ordering::SeqCst);
    let was_disabled = win::enable_window(owner, false);
    let disabled_now = !win::is_window_enabled(owner);
    // Enable it again later only if this process disabled it.
    OWNER_DISABLED.store(!was_disabled && disabled_now, Ordering::SeqCst);
    log!("settings: owner attached; disabled before {was_disabled}, now {disabled_now}");
}

/// Enables the owner again and brings it to the foreground. Safe to call
/// any number of times from anywhere: only the first call acts.
fn release_owner(why: &str) {
    let owner = OWNER.swap(0, Ordering::SeqCst);
    if owner == 0 {
        return;
    }
    let owner = owner as HWND;
    if !win::is_window(owner) {
        log!("settings: release ({why}): the owner is gone");
        return;
    }
    if OWNER_DISABLED.swap(false, Ordering::SeqCst) {
        win::enable_window(owner, true);
    }
    // Windows may refuse (spike S2); the result is logged for the hardware check.
    let foreground = win::set_foreground(owner);
    log!(
        "settings: release ({why}): owner enabled {}, SetForegroundWindow {foreground}",
        win::is_window_enabled(owner)
    );
}

/// SEC-2: DX12 only and the Fxc shader compiler, so that no `dxcompiler.dll`
/// is searched for, and no `WGPU_*` environment variable has any effect.
fn wgpu_configuration() -> eframe::egui_wgpu::WgpuConfiguration {
    // Why: `without_display_handle()` fills its defaults from WGPU_*
    // environment variables (egui-wgpu 0.37 setup.rs). Everything it read is
    // replaced below: the instance descriptor by one built from fixed
    // defaults (`new_without_display_handle()` reads no environment variable
    // in wgpu-types 30.0.1), then the backend, the compiler and the power
    // preference are set explicitly (as in spike S2).
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::DX12;
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .shader_compiler = wgpu::Dx12Compiler::Fxc;
    setup.power_preference = wgpu::PowerPreference::LowPower;
    eframe::egui_wgpu::WgpuConfiguration {
        wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

/// Loads a system font, as egui's bundled fonts are not used.
///
/// Why: decided in ADR-0024 (E-5): Segoe UI, then Arial,
/// from the fonts folder. Without either, text shows as boxes.
fn install_fonts(ctx: &egui::Context) {
    let folder = win::fonts_folder().unwrap_or_else(|| PathBuf::from(r"C:\Windows\Fonts"));
    for name in ["segoeui.ttf", "arial.ttf"] {
        match std::fs::read(folder.join(name)) {
            Ok(bytes) => {
                let mut fonts = egui::FontDefinitions::default();
                fonts.font_data.insert(
                    "system-ui".to_owned(),
                    Arc::new(egui::FontData::from_owned(bytes)),
                );
                for family in [FontFamily::Proportional, FontFamily::Monospace] {
                    fonts
                        .families
                        .entry(family)
                        .or_default()
                        .insert(0, "system-ui".to_owned());
                }
                ctx.set_fonts(fonts);
                log!("settings: font {name} loaded");
                return;
            }
            Err(e) => log!("settings: font {name} could not be read: {e}"),
        }
    }
    log!("settings: no font could be loaded; text will not show");
}

/// The dark style of the approved mockup.
fn apply_style(ctx: &egui::Context) {
    ctx.set_theme(Theme::Dark);
    ctx.style_mut_of(Theme::Dark, |style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Heading, FontId::proportional(18.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        let spacing = &mut style.spacing;
        spacing.item_spacing = vec2(8.0, 8.0);
        spacing.button_padding = vec2(16.0, 6.0);
        spacing.interact_size.y = ROW_HEIGHT;
        spacing.slider_width = 220.0;
        spacing.combo_width = 220.0;

        let visuals = &mut style.visuals;
        visuals.panel_fill = PANEL;
        visuals.window_fill = PANEL;
        visuals.window_stroke = Stroke::new(1.0, LINE);
        visuals.extreme_bg_color = WIDGET;
        visuals.faint_bg_color = WIDGET;
        visuals.weak_text_color = Some(DIM);
        visuals.hyperlink_color = ACCENT;
        visuals.selection.bg_fill = ACCENT;
        visuals.selection.stroke = Stroke::new(1.0, ACCENT_INK);
        visuals.slider_trailing_fill = true;
        let corner = CornerRadius::same(4);
        let widgets = &mut visuals.widgets;
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        for (state, fill, text) in [
            (&mut widgets.inactive, WIDGET, TEXT),
            (&mut widgets.hovered, WIDGET_HOVER, TEXT),
            (&mut widgets.active, WIDGET_HOVER, ACCENT),
            (&mut widgets.open, WIDGET_HOVER, TEXT),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::NONE;
            state.fg_stroke = Stroke::new(1.5, text);
            state.corner_radius = corner;
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Flight,
    Display,
    About,
}

struct SettingsApp {
    /// The values shown, saved on OK.
    form: Config,
    tab: Tab,
    location: Result<Location, String>,
    /// What to tell the user before OK replaces the file, if anything
    /// (a newer `schema_version`, F-08, or a file that could not be read).
    overwrite_warning: Option<String>,
    /// When that warning was shown.
    warned_at: Option<Instant>,
    /// Why the last save failed.
    error: Option<String>,
    /// The third-party license notices, if they could be read.
    notices: Option<String>,
}

impl SettingsApp {
    /// The eframe creation callback: fonts and style, then the owner.
    fn new(
        cc: &eframe::CreationContext<'_>,
        startup: Startup,
        owner: Option<HWND>,
        notices: Option<String>,
    ) -> Self {
        log!("settings: window created");
        install_fonts(&cc.egui_ctx);
        apply_style(&cc.egui_ctx);
        attach_owner(cc, owner);
        Self {
            form: startup.loaded.config,
            tab: Tab::Flight,
            location: startup.location,
            overwrite_warning: storage::overwrite_warning(
                &startup.loaded.source,
                startup.loaded.newer_schema,
            ),
            warned_at: None,
            error: None,
            notices,
        }
    }

    fn close(&self, ctx: &egui::Context, why: &str) {
        log!("settings: {why}");
        release_owner(why);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn ok(&mut self, ctx: &egui::Context) {
        // Why: decided in ADR-0024 (E-3): the first OK
        // on a newer or unreadable file only shows the warning; the second
        // one saves.
        if self.overwrite_warning.is_some() {
            match self.warned_at {
                None => {
                    self.warned_at = Some(Instant::now());
                    return;
                }
                // A double click on OK must not save unread (review L-A).
                Some(at) if !storage::second_ok_confirms(at.elapsed()) => return,
                Some(_) => {}
            }
        }
        let result = match &self.location {
            Ok(location) => {
                storage::write(&location.path, &config::to_toml(&self.form)).map_err(|e| {
                    format!(
                        "Could not save the settings to {}: {e}",
                        location.path.display()
                    )
                })
            }
            // Why: decided in ADR-0024 (E-6).
            Err(reason) => Err(format!("Could not save the settings: {reason}.")),
        };
        match result {
            Ok(()) => self.close(ctx, "saved; closing"),
            Err(message) => {
                log!("settings: saving failed");
                self.error = Some(message);
            }
        }
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.add_space(16.0);
        ui.label(RichText::new("Flying Microwave").heading().color(TEXT));
        ui.label(RichText::new("Screen saver settings").small().color(DIM));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (tab, name) in [
                (Tab::Flight, "Flight"),
                (Tab::Display, "Display"),
                (Tab::About, "About"),
            ] {
                let selected = self.tab == tab;
                let text = RichText::new(name).color(if selected { TEXT } else { DIM });
                let button = Button::new(text)
                    .fill(if selected {
                        PANEL
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(Stroke::NONE)
                    .corner_radius(CornerRadius {
                        nw: 6,
                        ne: 6,
                        sw: 0,
                        se: 0,
                    })
                    .min_size(vec2(0.0, 32.0));
                let response = ui.add(button);
                if selected {
                    ui.painter().hline(
                        response.rect.x_range(),
                        response.rect.bottom() - 1.0,
                        Stroke::new(2.0, ACCENT),
                    );
                }
                if response.clicked() {
                    self.tab = tab;
                }
            }
        });
    }

    fn footer(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if let Some(error) = &self.error {
            ui.add(Label::new(RichText::new(error).color(ui.visuals().error_fg_color)).wrap());
        } else if let (Some(text), Some(_)) = (&self.overwrite_warning, self.warned_at) {
            ui.add(
                Label::new(RichText::new(text.as_str()).color(ui.visuals().warn_fg_color)).wrap(),
            );
        }
        ui.horizontal(|ui| {
            if ui.button("Defaults").clicked() {
                self.form = Config::default();
                self.error = None;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Cancel").clicked() {
                    self.close(ctx, "Cancel clicked");
                }
                // The confirming OK reads "Replace", so that it does not look
                // like the same click again.
                let label = if self.overwrite_warning.is_some() && self.warned_at.is_some() {
                    "Replace"
                } else {
                    "OK"
                };
                let ok = Button::new(RichText::new(label).color(ACCENT_INK).strong()).fill(ACCENT);
                if ui.add(ok).clicked() {
                    self.ok(ctx);
                }
            });
        });
    }

    fn flight(&mut self, ui: &mut Ui) {
        let flight = &mut self.form.flight;
        row(ui, "Number of microwaves", None, |ui| {
            ui.add(Slider::new(&mut flight.count, COUNT_RANGE));
        });
        row(ui, "Speed", None, |ui| {
            ui.add(multiplier(&mut flight.speed, SPEED_RANGE));
        });
        row(ui, "Size", None, |ui| {
            ui.add(multiplier(&mut flight.scale, SCALE_RANGE));
        });
        row(
            ui,
            "Flight style",
            Some("With \"Level\", all microwaves face the same way."),
            |ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                ui.radio_value(&mut flight.style, Style::Tumbling, "Tumbling");
                ui.radio_value(&mut flight.style, Style::Level, "Level");
            },
        );
        // Two decimals, so that the file shows 1.15 rather than 1.1500000000000001.
        flight.speed = (flight.speed * 100.0).round() / 100.0;
        flight.scale = (flight.scale * 100.0).round() / 100.0;
    }

    fn display(&mut self, ui: &mut Ui) {
        let display = &mut self.form.display;
        row(ui, "Frame rate limit", None, |ui| {
            egui::ComboBox::from_id_salt("max_fps")
                .selected_text(fps_label(display.max_fps))
                .show_ui(ui, |ui| {
                    for choice in MaxFps::ALL {
                        ui.selectable_value(&mut display.max_fps, choice, fps_label(choice));
                    }
                });
        });
        row(
            ui,
            "Monitors",
            Some("With \"Primary only\", other monitors are covered in black."),
            |ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                ui.radio_value(&mut display.monitors, Monitors::All, "All monitors");
                ui.radio_value(&mut display.monitors, Monitors::Primary, "Primary only");
            },
        );
        let input = &mut self.form.input;
        row(
            ui,
            "Mouse sensitivity",
            Some("How far the mouse must move to end the screen saver."),
            |ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                for (choice, name) in ExitSensitivity::ALL
                    .into_iter()
                    .zip(["Low", "Medium", "High"])
                {
                    ui.radio_value(&mut input.exit_sensitivity, choice, name);
                }
            },
        );
    }

    fn about(&self, ui: &mut Ui) {
        // Why: the tab is only about 225 points high. With rows of the text
        // height, tighter spacing and the small text below, the notices get
        // about 110 to 125 of them (measured headlessly with egui 0.37 and
        // Arial-sized and wider fonts); with the spacing of the other tabs,
        // about 60, which overlapped the rows below.
        ui.spacing_mut().interact_size.y = 0.0;
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Version").color(DIM));
            ui.label(env!("CARGO_PKG_VERSION"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("License").color(DIM));
            ui.label(format!(
                "MIT. Third-party licenses are listed in {}.",
                licenses::FILE_NAME
            ));
        });
        // The rows under the notices are laid out from the bottom up, so that
        // the notices take whatever height is left between them.
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            // Why: shown as text; opening links is off (plan 0004, E-5).
            ui.label(RichText::new(PROJECT_URL).color(ACCENT));
            ui.add(
                Label::new(
                    RichText::new(
                        "Inspired by the classic flying-appliance screen savers. This is an \
                         independent work, not affiliated with or endorsed by their creators \
                         or rights holders.",
                    )
                    .small(),
                )
                .wrap(),
            );
            ui.with_layout(Layout::top_down(Align::Min), |ui| match &self.notices {
                Some(text) => notices(ui, text),
                // The release puts the file next to the program; without it,
                // say so and name the file (plan 0004, D-12).
                None => {
                    ui.add(
                        Label::new(
                            RichText::new(format!(
                                "{} could not be read from the folder of the program.",
                                licenses::FILE_NAME
                            ))
                            .small()
                            .color(DIM),
                        )
                        .wrap(),
                    );
                }
            });
        });
    }
}

/// One labeled row: the label on the left, the controls and an optional
/// hint on the right.
fn row(ui: &mut Ui, label: &str, hint: Option<&str>, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(LABEL_WIDTH, ROW_HEIGHT),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_size(vec2(LABEL_WIDTH, ROW_HEIGHT));
                ui.label(label);
            },
        );
        ui.vertical(|ui| {
            ui.horizontal(add);
            if let Some(hint) = hint {
                ui.add(Label::new(RichText::new(hint).small().color(DIM)).wrap());
            }
        });
    });
    ui.add_space(10.0);
}

/// The third-party license notices (plan 0004, F-05 and D-12), in a framed
/// area that fills the height left and scrolls; long lines wrap.
fn notices(ui: &mut Ui, text: &str) {
    Frame::NONE
        .fill(BG)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                // Why: the default minimum (64) would grow the area over the
                // rows below it when less height is left (larger fonts).
                .min_scrolled_height(0.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(Label::new(RichText::new(text).small().color(DIM)).wrap());
                });
        });
}

/// A slider for a multiplier, shown like `1.00×`.
fn multiplier(value: &mut f64, range: std::ops::RangeInclusive<f64>) -> Slider<'_> {
    Slider::new(value, range)
        .step_by(0.05)
        .fixed_decimals(2)
        .suffix("×")
}

fn fps_label(fps: MaxFps) -> String {
    match fps {
        MaxFps::MatchMonitor => "Match monitor".to_owned(),
        other => format!("{} fps", other.value()),
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // The title bar button or Alt+F4: release the owner while our window
        // still exists, then eframe closes it.
        if ctx.input(|i| i.viewport().close_requested()) {
            log!("settings: close requested");
            release_owner("close requested");
        }
        egui::Panel::top("header")
            .frame(Frame::NONE.fill(BG).inner_margin(Margin {
                left: 16,
                right: 16,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| self.header(ui));
        egui::Panel::bottom("footer")
            .frame(Frame::NONE.fill(BG).inner_margin(Margin::same(16)))
            .show(ui, |ui| self.footer(ui, &ctx));
        egui::CentralPanel::default_margins()
            .frame(Frame::NONE.fill(PANEL).inner_margin(Margin {
                left: 20,
                right: 20,
                top: 18,
                bottom: 8,
            }))
            .show(ui, |ui| match self.tab {
                Tab::Flight => self.flight(ui),
                Tab::Display => self.display(ui),
                Tab::About => self.about(ui),
            });
    }

    fn on_exit(&mut self) {
        release_owner("on_exit");
    }
}
