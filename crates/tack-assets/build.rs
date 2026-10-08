//! Package the pinned, unmodified decoder tool. No downloading or compiling here.
use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let suffix = if env::var("CARGO_CFG_TARGET_OS")? == "windows" {
        ".exe"
    } else {
        ""
    };
    let native = manifest.join(format!(
        "../../target/native/libjpeg-turbo-3.2.0/bin/djpeg{suffix}"
    ));
    println!("cargo:rerun-if-changed={}", native.display());
    let bytes = fs::read(&native)
        .map_err(|e| format!("prepare pinned djpeg with tools/prepare_turbojpeg.py: {e}"))?;
    let output = PathBuf::from(env::var("OUT_DIR")?);
    let profile = output
        .ancestors()
        .nth(3)
        .ok_or("unexpected Cargo output directory")?;
    for directory in [
        profile.to_owned(),
        profile.join("deps"),
        profile.join("examples"),
    ] {
        fs::create_dir_all(&directory)?;
        let target = directory.join(format!("tack-jpeg-decoder{suffix}"));
        if fs::read(&target).ok().as_deref() != Some(bytes.as_slice()) {
            fs::copy(&native, target)?;
        }
    }
    Ok(())
}
