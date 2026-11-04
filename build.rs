#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return;
    }

    let out_dir = match env::var_os("OUT_DIR") {
        Some(d) => PathBuf::from(d),
        None => return,
    };

    let pkg_ver = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let mut nums = pkg_ver
        .split('.')
        .map(|s| s.parse::<u16>().unwrap_or(0))
        .collect::<Vec<_>>();
    while nums.len() < 4 {
        nums.push(0);
    }

    let publisher = env::var("LEKH_PUBLISHER").unwrap_or_else(|_| "khajumsanjog".to_string());
    let rc_path = out_dir.join("lekh_version.rc");
    let res_path = out_dir.join("lekh_version.res");

    let rc_contents = format!(
        r#"1 VERSIONINFO
FILEVERSION {v0},{v1},{v2},{v3}
PRODUCTVERSION {v0},{v1},{v2},{v3}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "{pub_name}\0"
            VALUE "FileDescription", "Lekh (lk) - Fast, safe terminal text editor\0"
            VALUE "FileVersion", "{ver}.0\0"
            VALUE "InternalName", "lekh\0"
            VALUE "LegalCopyright", "Copyright (c) 2026 {pub_name} (MIT License)\0"
            VALUE "ProductName", "Lekh\0"
            VALUE "ProductVersion", "{ver}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        v0 = nums[0],
        v1 = nums[1],
        v2 = nums[2],
        v3 = nums[3],
        pub_name = publisher.replace('"', ""),
        ver = pkg_ver,
    );

    if fs::write(&rc_path, rc_contents).is_err() {
        return;
    }

    let status = Command::new("rc.exe")
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .status();

    if let Ok(s) = status {
        if s.success() && res_path.exists() {
            println!("cargo:rustc-link-arg-bins={}", res_path.display());
        }
    }
}

