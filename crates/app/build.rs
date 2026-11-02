//! Embeds the Windows resources (icon, version information and the screen
//! saver description) into the executable (plan 0002, T-1), and sets the
//! linker option of SEC-1 that restricts the import search path.

use std::env;
use std::fs;
use std::path::PathBuf;

const RESOURCES: &str = "res/app.rc";

fn main() {
    // Why: embed-resource does not report its inputs to Cargo, so without
    // these lines a changed .rc or icon would keep the old resources.
    println!("cargo:rerun-if-changed={RESOURCES}");
    println!("cargo:rerun-if-changed=res/flying-microwave.ico");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    fs::write(out_dir.join("fm_version.h"), version_header()).expect("write fm_version.h");

    // Why: rc.exe runs in the crate directory, not in res/; the include
    // path lets it find the icon named in the .rc file.
    let res_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by Cargo")).join("res");
    // SEC-1 (docs/design/architecture.md): resolve the DLLs the executable
    // imports statically (dxgi.dll, user32.dll, ...) from System32 only, so
    // a DLL planted next to the .scr is not loaded before main() runs.
    // 0x800 is LOAD_LIBRARY_SEARCH_SYSTEM32 (MSVC linker reference,
    // /DEPENDENTLOADFLAG). Only the executable gets it, as with the resources.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }

    let result = embed_resource::compile(RESOURCES, embed_resource::ParamsIncludeDirs([res_dir]));
    // Why: on a Windows host a missing resource compiler must fail the
    // build, or a release would ship without version information. On other
    // hosts (type checks of the Windows target on Linux) the resources are
    // optional, as llvm-rc may not be installed.
    let checked = if cfg!(windows) {
        result.manifest_required()
    } else {
        result.manifest_optional()
    };
    if let Err(e) = checked {
        panic!("{e}");
    }
}

/// The version from Cargo.toml in the forms VERSIONINFO needs.
fn version_header() -> String {
    let part = |name: &str| env::var(name).unwrap_or_else(|_| panic!("{name} is set by Cargo"));
    let (major, minor, patch) = (
        part("CARGO_PKG_VERSION_MAJOR"),
        part("CARGO_PKG_VERSION_MINOR"),
        part("CARGO_PKG_VERSION_PATCH"),
    );
    let version = part("CARGO_PKG_VERSION");
    // VS_FF_PRERELEASE for versions such as 0.1.0-alpha.3.
    let flags = if part("CARGO_PKG_VERSION_PRE").is_empty() {
        "0x0L"
    } else {
        "0x2L"
    };
    // Why: the numeric version has four parts; the fourth is always 0, as
    // a pre-release is marked by FM_FILEFLAGS and the version string.
    format!(
        "#define FM_VERSION_BIN {major},{minor},{patch},0\n\
         #define FM_VERSION_STR \"{version}\"\n\
         #define FM_FILEFLAGS {flags}\n"
    )
}
