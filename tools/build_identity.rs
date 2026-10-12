// Shared compile-time identity, independent of network/runtime state.
// Git's resolved paths also track linked worktrees and non-dev branch names.
fn emit_build_identity(root: &std::path::Path) {
    let git = |arguments: &[&str]| {
        std::process::Command::new("git")
            .args(arguments).current_dir(root).output().ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_owned())
    };
    let mut files = vec!["HEAD".to_owned(), "index".to_owned(), "packed-refs".to_owned()];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) { files.push(branch); }
    for file in files {
        if let Some(path) = git(&["rev-parse", "--path-format=absolute", "--git-path", &file]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    println!("cargo:rerun-if-env-changed=TACK_BUILD_CHANNEL");
    let sha = git(&["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=TACK_GIT_SHA={sha}");
    println!("cargo:rustc-env=TACK_BUILD_CHANNEL={}", std::env::var("TACK_BUILD_CHANNEL").unwrap_or_else(|_| "dev".into()));
}
