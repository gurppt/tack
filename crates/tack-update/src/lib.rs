//! Small pure update contract. HTTPS, archive extraction and process replacement live in the helper.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub const REPOSITORY: &str = "gurppt/tack";
pub const MAX_MANIFEST: usize = 32 * 1024;
pub const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
pub const MAX_UNPACKED: u64 = 256 * 1024 * 1024;
pub const PROTOCOL_MAJOR: u32 = 2;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Dev,
    Stable,
}
impl Channel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dev => "Dev",
            Self::Stable => "Stable",
        }
    }
    pub fn accepts(self, version: &semver::Version) -> bool {
        match self {
            Self::Dev => version.pre.as_str().starts_with("dev."),
            Self::Stable => version.pre.is_empty(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub platform: String,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub channel: Channel,
    pub git_sha: String,
    pub protocol_major: u32,
    pub assets: Vec<Asset>,
}
impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_MANIFEST {
            return Err("Update manifest exceeds 32 KiB".into());
        }
        let manifest: Self = serde_json::from_slice(bytes)?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<(), Error> {
        let version = semver::Version::parse(&self.version)?;
        if self.schema != 1
            || self.protocol_major != PROTOCOL_MAJOR
            || self.version.len() > 64
            || !self.channel.accepts(&version)
            || !hex(&self.git_sha, 40)
            || !(1..=2).contains(&self.assets.len())
        {
            return Err("Unsupported update schema/version/channel/commit/protocol".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for asset in &self.assets {
            if !matches!(asset.platform.as_str(), "linux-x86_64" | "windows-x86_64")
                || asset.name != format!("tack-{}.zip", asset.platform)
                || !(1..=MAX_ARCHIVE).contains(&asset.size)
                || !hex(&asset.sha256, 64)
                || !seen.insert(&asset.platform)
            {
                return Err("Invalid platform asset identity, size or checksum".into());
            }
        }
        Ok(())
    }
    pub fn newer_than(&self, current: &str) -> Result<bool, Error> {
        Ok(semver::Version::parse(&self.version)? > semver::Version::parse(current)?)
    }
    pub fn asset(&self, platform: &str) -> Result<&Asset, Error> {
        self.assets
            .iter()
            .find(|a| a.platform == platform)
            .ok_or_else(|| "No update for this platform".into())
    }
    pub fn url(&self, name: &str) -> String {
        format!(
            "https://github.com/{REPOSITORY}/releases/download/v{}/{name}",
            self.version
        )
    }
}
fn hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|v| v.is_ascii_hexdigit() && !v.is_ascii_uppercase())
}
pub fn read_small(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("Update metadata size limit".into());
    }
    Ok(bytes)
}
pub fn platform() -> Result<&'static str, Error> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Ok("linux-x86_64")
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Ok("windows-x86_64")
    } else {
        Err("No portable update for this platform".into())
    }
}
pub fn verify(path: &Path, asset: &Asset) -> Result<(), Error> {
    let mut file = std::fs::File::open(path)?;
    if file.metadata()?.len() != asset.size {
        return Err("Update size mismatch".into());
    }
    let mut digest = Sha256::new();
    let mut chunk = [0; 64 * 1024];
    let mut size = 0;
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > asset.size {
            return Err("Update size exceeded".into());
        }
        digest.update(&chunk[..n]);
    }
    if size != asset.size || format!("{:x}", digest.finalize()) != asset.sha256 {
        return Err("Update checksum mismatch".into());
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub manifest: Manifest,
    pub notes: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Response {
    Applying,
    Current,
    Available(Offer),
    Staged {
        directory: std::path::PathBuf,
        manifest: Manifest,
    },
}
