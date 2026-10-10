include!("../../tools/build_identity.rs");
fn main() {
    emit_build_identity(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
}
