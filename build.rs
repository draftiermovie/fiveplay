//! Embeds every file under `assets/` into the executable (see
//! `src/embedded.rs`), so the game ships as a single `.exe`. Windows builds
//! also get the game icon and version details (the name Explorer, the
//! taskbar and Task Manager show).
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets");
    println!("cargo:rerun-if-changed=assets");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    let mut code = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let relative = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let absolute = file.to_string_lossy().to_string();
        code.push_str(&format!(
            "    ({relative:?}, include_bytes!({absolute:?})),\n"
        ));
    }
    code.push_str("];\n");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("assets.rs");
    fs::write(out, code).unwrap();
    windows_resources();
}

/// Icon (`packaging/windows/fiveplay.ico`, from `tools/make_icons.py`) and
/// version details for `FivePlay.exe`. Needs the Windows SDK's resource
/// compiler, which the Visual Studio C++ build tools include.
fn windows_resources() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let icon = "packaging/windows/fiveplay.ico";
    println!("cargo:rerun-if-changed={icon}");
    let mut res = winresource::WindowsResource::new();
    res.set_icon(icon)
        .set("ProductName", "FivePlay")
        .set("FileDescription", "FivePlay")
        .set("CompanyName", "FivePlay")
        .set("OriginalFilename", "FivePlay.exe");
    if let Err(err) = res.compile() {
        // A missing resource compiler only costs the icon, not the build.
        println!("cargo:warning=FivePlay.exe built without its icon: {err}");
    }
}
