//! Gives the executable a file icon. Windows-only and best effort: without a
//! resource compiler the build still succeeds, the .exe just shows the default
//! icon. The title bar / taskbar icon is set at runtime from the same PNG.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // rc.exe is the MSVC resource compiler; the GNU toolchain would need
    // windres and a different input format, so only msvc is handled.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/icon.ico");
    if !icon.is_file() {
        return;
    }
    let Some(rc) = find_rc() else {
        println!("cargo:warning=no rc.exe found, building without a file icon");
        return;
    };

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    // rc resolves the icon path against the current directory, so hand it a
    // generated .rc that names the icon absolutely.
    let script = out.join("icon.rc");
    // rc.exe treats \a in a path as an escape, so hand it forward slashes.
    let icon_path = icon.display().to_string().replace('\\', "/");
    write(&script, &format!("1 ICON \"{icon_path}\"\n"));
    let resource = out.join("icon.res");
    let status = Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg(&script)
        .status();
    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg={}", resource.display());
        }
        _ => println!("cargo:warning=rc.exe failed, building without a file icon"),
    }
}

fn write(path: &Path, text: &str) {
    if std::fs::write(path, text).is_err() {
        println!("cargo:warning=could not write {}", path.display());
    }
}

/// `rc.exe` from the Windows SDK, looked for the way the linker is found: the
/// newest SDK, in the target's architecture.
fn find_rc() -> Option<PathBuf> {
    if let Some(found) = on_path() {
        return Some(found);
    }
    let kits = std::env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"))
        .join("Windows Kits/10/bin");
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86") => "x86",
        _ => "x64",
    };
    let mut versions: Vec<PathBuf> = std::fs::read_dir(&kits)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join(arch).join("rc.exe").is_file())
        .collect();
    versions.sort();
    versions.pop().map(|path| path.join(arch).join("rc.exe"))
}

fn on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("rc.exe"))
        .find(|candidate| candidate.is_file())
}
