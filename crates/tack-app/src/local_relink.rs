//! Explicit validated replacement; identity/layout survive and source revision never recycles.
use std::path::Path;
use tack_assets::{AssetError, image_metadata, source_fingerprint};
use tack_core::*;
pub struct RelinkReady {
    pub previous: Source,
    pub path: LinkedPath,
    pub fingerprint: SourceFingerprint,
    pub pixels: [u32; 2],
}
impl RelinkReady {
    pub fn read(previous: Source, path: &Path) -> Result<Self, AssetError> {
        let path = path.canonicalize()?;
        let fingerprint = source_fingerprint(&path)?;
        if fingerprint.size > crate::local_import::MAX_ENCODED_IMAGE {
            return Err("replacement exceeds 64 MiB encoded-input limit".into());
        }
        let (pixels, _) = image_metadata(&path)?;
        if source_fingerprint(&path)? != fingerprint {
            return Err("replacement changed during admission".into());
        }
        Ok(Self {
            previous,
            path: LinkedPath::native(&path)?,
            fingerprint,
            pixels,
        })
    }
    pub fn apply(self, editor: &mut DocumentEditor) -> Result<bool, AssetError> {
        let id = self.previous.id();
        if editor.document().source(id) != Some(&self.previous) {
            return Err("selected source changed while relink was open".into());
        }
        let revision = editor
            .next_source_revision()
            .ok_or("source revision exhausted")?;
        let mut commands = vec![Command::SetSource(Source::from_descriptor(
            id,
            SourceLocation::Linked(self.path),
            revision,
            Some(self.fingerprint),
        )?)];
        for asset in editor.document().assets().filter(|a| a.source_id() == id) {
            commands.push(Command::SetAsset(ImageAsset::new(
                asset.id(),
                id,
                self.pixels,
            )?));
        }
        Ok(editor.execute(Command::Batch(commands))?)
    }
}
