//! The eframe settings window and the Win32 owner/modal handling (spike S2).
//!
//! eframe 0.37 has no API for a native owner window, but `CreationContext`
//! implements `HasWindowHandle`. So the owner is attached right after the
//! window exists: `SetWindowLongPtrW(GWLP_HWNDPARENT)` makes it the owner and
//! `EnableWindow(owner, FALSE)` makes the owner's dialog inert (modal
//! behavior). On every way out the owner is enabled again *before* the window
//! goes away, so that Windows does not activate some unrelated window.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use eframe::egui;
use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};

use crate::log::log;
use crate::win;

/// Window handle of the owner (0 = none).
static OWNER: AtomicUsize = AtomicUsize::new(0);
/// True while this process has the owner disabled and still has to enable it.
static OWNER_DISABLED_BY_US: AtomicBool = AtomicBool::new(false);

/// Fonts tried in order. The system font is loaded at runtime because egui's
/// bundled fonts (feature `default_fonts`) are not OSI licensed.
const FONT_CANDIDATES: [&str; 2] = [
    r"C:\Windows\Fonts\segoeui.ttf",
    r"C:\Windows\Fonts\arial.ttf",
];

/// Remembers the owner window that was resolved from the command line.
pub fn set_owner_candidate(owner: usize) {
    OWNER.store(owner, Ordering::SeqCst);
}

/// Re-enables the owner and gives it the foreground again. Safe to call any
/// number of times and from any place (button, close request, `on_exit`,
/// after `run_native` returned, panic hook): only the first call acts.
pub fn release_owner(why: &str) {
    if !OWNER_DISABLED_BY_US.swap(false, Ordering::SeqCst) {
        log!("release_owner({why}): nothing to do (owner not disabled by us, or already released)");
        return;
    }
    let owner_value = OWNER.load(Ordering::SeqCst);
    let owner = win::hwnd_from_usize(owner_value);
    log!("release_owner({why}): owner HWND {owner_value}");
    let valid = win::is_window(owner);
    log!("  IsWindow(owner) = {valid}");
    if !valid {
        return;
    }
    let was_disabled = win::enable_window(owner, true);
    log!(
        "  EnableWindow(owner, TRUE): owner was disabled before = {was_disabled}; \
         IsWindowEnabled(owner) now = {}",
        win::is_window_enabled(owner)
    );
    let foreground = win::set_foreground(owner);
    log!("  SetForegroundWindow(owner) = {foreground}");
}

/// Builds the wgpu configuration with the hardening of SEC-2: DX12 only and
/// the Fxc shader compiler (so that no `dxcompiler.dll` is ever searched).
pub fn wgpu_configuration() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::wgpu::{Backends, Dx12Compiler, InstanceDescriptor, PowerPreference};

    // Why: `without_display_handle()` fills its defaults from WGPU_* environment
    // variables (backends, instance flags, backend options, power preference;
    // egui-wgpu 0.37 setup.rs). Everything it read is replaced below: the
    // instance descriptor wholesale by one built from fixed defaults
    // (`new_without_display_handle()` reads no environment variables in
    // wgpu-types 30.0.1), then backends and compiler are set explicitly, and so
    // is the power preference. The device descriptor is not affected by the
    // environment. So no WGPU_* variable of the process environment can change
    // the backend, the shader compiler or the adapter choice.
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor = InstanceDescriptor::new_without_display_handle();
    setup.instance_descriptor.backends = Backends::DX12;
    setup
        .instance_descriptor
        .backend_options
        .dx12
        .shader_compiler = Dx12Compiler::Fxc;
    setup.power_preference = PowerPreference::LowPower;
    eframe::egui_wgpu::WgpuConfiguration {
        wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

/// Installs a system font as the first font of every family. Without any
/// font egui cannot draw text (it falls back to boxes), so failure is only logged.
fn install_fonts(ctx: &egui::Context) {
    for path in FONT_CANDIDATES {
        match std::fs::read(path) {
            Ok(bytes) => {
                log!("font loaded from {path} ({} bytes)", bytes.len());
                let mut fonts = egui::FontDefinitions::default();
                fonts.font_data.insert(
                    "system-ui".to_owned(),
                    Arc::new(egui::FontData::from_owned(bytes)),
                );
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    fonts
                        .families
                        .entry(family)
                        .or_default()
                        .insert(0, "system-ui".to_owned());
                }
                ctx.set_fonts(fonts);
                return;
            }
            Err(e) => log!("font {path} could not be read: {e}"),
        }
    }
    log!("no system font could be loaded; text may not render (boxes instead of glyphs)");
}

/// Makes the already resolved owner window the owner of the settings window
/// and disables it. Every step and its result is logged.
fn attach_owner(cc: &eframe::CreationContext<'_>) {
    let owner_value = OWNER.load(Ordering::SeqCst);
    if owner_value == 0 {
        log!("owner: none, the settings window is a plain top-level window");
        return;
    }
    let owner = win::hwnd_from_usize(owner_value);
    // Re-validate: time has passed since the command line was parsed.
    if !win::is_window(owner) {
        log!("owner: HWND {owner_value} is no longer a window; continuing without an owner");
        return;
    }

    let window = match cc.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(handle)) => {
            win::hwnd_from_usize(handle.hwnd.get().cast_unsigned())
        }
        Ok(other) => {
            log!("owner: unexpected raw window handle {other:?}; continuing without an owner");
            return;
        }
        Err(e) => {
            log!("owner: no window handle from CreationContext ({e}); continuing without an owner");
            return;
        }
    };
    log!(
        "owner: settings window HWND = {}, owner HWND = {owner_value}",
        win::hwnd_to_usize(window)
    );

    match win::set_owner(window, owner) {
        Ok(previous) => {
            log!("  SetWindowLongPtrW(GWLP_HWNDPARENT) succeeded, previous value {previous}")
        }
        Err(code) => {
            log!(
                "  SetWindowLongPtrW(GWLP_HWNDPARENT) FAILED, GetLastError={code}; continuing without an owner"
            );
            return;
        }
    }
    log!(
        "  GetWindow(GW_OWNER) now = {} (expected {owner_value})",
        win::hwnd_to_usize(win::owner_of(window))
    );

    let was_disabled = win::enable_window(owner, false);
    let enabled_now = win::is_window_enabled(owner);
    log!(
        "  EnableWindow(owner, FALSE): owner was disabled before = {was_disabled}; \
         IsWindowEnabled(owner) now = {enabled_now} (expected false); GetLastError={}",
        win::last_error()
    );
    // Only remember to re-enable if we really disabled it.
    OWNER_DISABLED_BY_US.store(!was_disabled && !enabled_now, Ordering::SeqCst);
    if was_disabled {
        log!("  owner was already disabled before us; it will be left disabled");
    }
}

pub struct SettingsApp {
    microwaves: u32,
    show_clock: bool,
}

impl SettingsApp {
    /// The `eframe` creation callback: fonts first, then the owner handling.
    pub fn create(cc: &eframe::CreationContext<'_>) -> Self {
        log!("eframe creation callback running");
        install_fonts(&cc.egui_ctx);
        attach_owner(cc);
        Self {
            microwaves: 10,
            show_clock: true,
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // The title bar close button (or Alt+F4): release the owner while our
        // window still exists, then let eframe close it.
        if ctx.input(|i| i.viewport().close_requested()) {
            log!("close requested by the window (title bar button or Alt+F4)");
            release_owner("close requested");
        }

        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.add(egui::Slider::new(&mut self.microwaves, 1..=50).text("Number of microwaves"));
            ui.checkbox(&mut self.show_clock, "Show clock on display");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    log!(
                        "OK clicked (nothing is saved): microwaves={}, show_clock={}",
                        self.microwaves,
                        self.show_clock
                    );
                    release_owner("OK clicked");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Cancel").clicked() {
                    log!("Cancel clicked");
                    release_owner("Cancel clicked");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }

    fn on_exit(&mut self) {
        log!("App::on_exit");
        release_owner("on_exit");
    }
}
