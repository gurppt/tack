// Shared compile-time identity, independent of network/runtime state.
fn emit_build_identity(root: &std::path::Path) {
    for file in [".git/HEAD", ".git/refs/heads/dev", ".git/refs/heads/main", ".git/index"] {
        println!("cargo:rerun-if-changed={}", root.join(file).display());
    }
    println!("cargo:rerun-if-env-changed=TACK_BUILD_CHANNEL");
    let sha=std::process::Command::new("git").args(["rev-parse","HEAD"]).current_dir(root).output()
        .ok().filter(|o|o.status.success()).and_then(|o|String::from_utf8(o.stdout).ok())
        .map(|s|s.trim().to_owned()).unwrap_or_else(||"unknown".into());
    println!("cargo:rustc-env=TACK_GIT_SHA={sha}");
    println!("cargo:rustc-env=TACK_BUILD_CHANNEL={}",std::env::var("TACK_BUILD_CHANNEL").unwrap_or_else(|_|"dev".into()));
}
