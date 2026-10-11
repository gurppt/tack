//! Explicit source replacement uses the existing finite, cancellable I/O worker.
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tack_assets::{AssetError, image_metadata, source_fingerprint};
use tack_core::*;
use tack_storage::Payload;
pub struct Request {
    pub document: DocumentId,
    pub previous: Source,
    pub board: PathBuf,
    pub work: PathBuf,
    pub spool: Option<Arc<std::fs::File>>,
    pub toggle: bool,
}
pub struct Ready {
    pub document: DocumentId,
    pub previous: Source,
    pub location: SourceLocation,
    pub origin: LinkedPath,
    pub fingerprint: SourceFingerprint,
    pub pixels: [u32; 2],
    pub original: Option<Payload>,
}
impl Request {
    pub fn run(mut self, cancel: &AtomicBool) -> Result<Option<Ready>, AssetError> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let linked = matches!(self.previous.location(), SourceLocation::Linked(_));
        let path = self
            .previous
            .original_path()
            .and_then(|p| p.to_native())
            .map(|p| {
                if p.is_absolute() {
                    p
                } else {
                    self.board
                        .parent()
                        .unwrap_or(std::path::Path::new("."))
                        .join(p)
                }
            });
        let path = if let Some(p) = path.filter(|p| p.is_file()) {
            p
        } else if self.toggle && !linked {
            let paths = crate::native_files::pick(
                crate::native_files::Picker::Relink,
                &self.work.join("picker"),
                cancel,
            )?;
            let Some(p) = paths.into_iter().next() else {
                return Ok(None);
            };
            p
        } else {
            return Err("Original local file unavailable; use Relink Source".into());
        };
        let path = path.canonicalize()?;
        let fingerprint = source_fingerprint(&path)?;
        if fingerprint.size > crate::local_import::MAX_ENCODED_IMAGE {
            return Err("Source exceeds 64 MiB encoded limit".into());
        }
        let (pixels, _) = image_metadata(&path)?;
        let embedded = if self.toggle { linked } else { !linked };
        let original = if embedded {
            tack_storage::create_private_directory(&self.work, true)?;
            Some(crate::local_import::pin_copy(
                &path,
                &self.work,
                &mut self.spool,
                cancel,
                &mut 0,
            )?)
        } else {
            None
        };
        if cancel.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if source_fingerprint(&path)? != fingerprint {
            return Err("Source changed during admission".into());
        }
        let origin = LinkedPath::native(&path)?;
        Ok(Some(Ready {
            document: self.document,
            previous: self.previous,
            location: if embedded {
                SourceLocation::Embedded
            } else {
                SourceLocation::Linked(origin.clone())
            },
            origin,
            fingerprint,
            pixels,
            original,
        }))
    }
}
impl Ready {
    /// Return the original's new revision only after one validated atomic edit.
    pub fn apply(
        self,
        editor: &mut DocumentEditor,
    ) -> Result<(SourceId, u64, Option<Payload>), AssetError> {
        if editor.document().id() != self.document
            || editor.document().source(self.previous.id()) != Some(&self.previous)
        {
            return Err("Source changed while operation was pending".into());
        }
        let revision = editor
            .next_source_revision()
            .ok_or("Source revision exhausted")?;
        let id = self.previous.id();
        let source = Source::from_descriptor(id, self.location, revision, Some(self.fingerprint))?
            .with_original_path(Some(self.origin));
        let mut edits = vec![Command::SetSource(source)];
        for asset in editor.document().assets().filter(|a| a.source_id() == id) {
            edits.push(Command::SetAsset(ImageAsset::new(
                asset.id(),
                id,
                self.pixels,
            )?));
        }
        editor.execute(Command::Batch(edits))?;
        Ok((id, revision, self.original))
    }
}
