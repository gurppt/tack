use std::path::PathBuf;
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new() -> Result<Self, tack_update::Error> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let p =
            std::env::temp_dir().join(format!("tack-updater-test-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
