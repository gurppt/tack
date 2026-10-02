use crate::AssetError;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tack_core::WorldRect;

#[derive(Deserialize)]
struct Manifest {
    schema: u32,
    objects: Vec<ManifestObject>,
}

#[derive(Deserialize)]
struct ManifestObject {
    id: u32,
    path: PathBuf,
    source_sha256: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

pub struct ImageObject {
    pub id: u32,
    pub path: PathBuf,
    pub source_sha256: String,
    pub rect: WorldRect,
}

/// Immutable geometry snapshot. A linear cull is the initial measured baseline;
/// no speculative index until its cost is established for the 1,000-object board.
pub struct Board {
    pub objects: Vec<ImageObject>,
}

impl Board {
    /// Startup boundary; call before entering the render/event loop.
    pub fn read_manifest(path: &Path) -> Result<Self, AssetError> {
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err("manifest exceeds 4 MiB".into());
        }
        let manifest: Manifest = serde_json::from_slice(&bytes)?;
        if manifest.schema != 1 || manifest.objects.is_empty() || manifest.objects.len() > 10000 {
            return Err("unsupported or oversized benchmark manifest".into());
        }
        let root = path
            .parent()
            .ok_or("manifest has no parent")?
            .canonicalize()?;
        let mut ids = HashSet::new();
        let mut objects = Vec::with_capacity(manifest.objects.len());
        for obj in manifest.objects {
            if !ids.insert(obj.id)
                || obj.source_sha256.len() != 64
                || !obj.source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("invalid asset ID or source hash".into());
            }
            let path = root.join(obj.path).canonicalize()?;
            if !path.starts_with(&root) {
                return Err("asset path escapes corpus".into());
            }
            objects.push(ImageObject {
                id: obj.id,
                path,
                source_sha256: obj.source_sha256,
                rect: WorldRect::new(obj.x, obj.y, obj.width, obj.height)?,
            });
        }
        Ok(Self { objects })
    }

    pub fn visible(&self, viewport: WorldRect) -> impl Iterator<Item = &ImageObject> {
        self.objects
            .iter()
            .filter(move |obj| obj.rect.intersects(viewport))
    }
}
