use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
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
