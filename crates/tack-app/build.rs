use std::fs;
use std::{env, error::Error, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../..");
    for file in [
        "Cargo.toml",
        "gfx/about.toml",
        "gfx/tack_about.png",
        "gfx/logo_tack_about.png",
        "tools/prepare_about.py",
        "tools/prepare_icon.py",
        "gfx/tack-icon.svg",
        "gfx/Rhombus--Streamline-Fluent-Ui-Filled.svg",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(file).display());
    }
    println!("cargo:rerun-if-env-changed=PYTHON");
    let output = PathBuf::from(env::var("OUT_DIR")?);
    let python = env::var_os("PYTHON")
        .unwrap_or_else(|| if cfg!(windows) { "python" } else { "python3" }.into());
    let status = Command::new(python)
        .arg(root.join("tools/prepare_about.py"))
        .arg("--root")
        .arg(&root)
        .arg("--output")
        .arg(&output)
        .arg("--version")
        .arg(env::var("CARGO_PKG_VERSION")?)
        .status()?;
    if !status.success() {
        return Err("About preparation failed: use Python 3.12+ with Pillow 10.2.0 and validate gfx/about.toml".into());
    }
    // Package only the compact PNG beside normal binaries and test executables.
    // No original artwork is embedded or copied into runtime packages.
    let profile = output
        .ancestors()
        .nth(3)
        .ok_or("unexpected Cargo output directory")?;
    let bytes = fs::read(output.join("about.png"))?;
    for directory in [profile.to_owned(), profile.join("deps")] {
        fs::create_dir_all(&directory)?;
        let asset = directory.join("tack-about.png");
        println!("cargo:rerun-if-changed={}", asset.display());
        if fs::read(&asset).ok().as_deref() != Some(bytes.as_slice()) {
            // Build/package preparation only; the ordinary app never reads here.
            fs::write(&asset, &bytes)?;
        }
    }
    let logo = fs::read(root.join("gfx/logo_tack_about.png"))?;
    for directory in [profile.to_owned(), profile.join("deps")] {
        let asset = directory.join("tack-about-logo.png");
        if fs::read(&asset).ok().as_deref() != Some(logo.as_slice()) {
            fs::write(&asset, &logo)?;
        }
    }
    for name in [
        "tack-icon-16.png",
        "tack-icon-32.png",
        "tack-icon-64.png",
        "tack-icon-128.png",
        "tack-icon-256.png",
        "tack-icon.ico",
    ] {
        let bytes = fs::read(output.join(name))?;
        let path = profile.join(name);
        if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
            fs::write(path, bytes)?;
        }
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        // Optional local libXi fixes startup reentrancy on older X11 distributions.
        // No download or native compilation is performed by Cargo itself.
        let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
        let library = manifest.join("../../target/native/libXi-1.8.3/lib");
        println!("cargo:rerun-if-changed={}/libXi.so.6", library.display());
        println!(
            "cargo:rustc-link-arg-bin=tack-app=-Wl,-rpath,{}",
            library.display()
        );
    }
    Ok(())
}
