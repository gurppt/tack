//! Header admission and original copying on one cancellable worker, never thumbnail preparation.
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tack_assets::{AssetError, image_metadata, source_fingerprint};
use tack_core::*;
use tack_storage::{BlobRange, Payload};
pub const MAX_IMPORT_FILES: usize = 4096;
pub const MAX_ENCODED_IMAGE: u64 = 64 * 1024 * 1024;
pub const MAX_IMPORT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const MAX_SESSION_IMPORT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

pub struct ImportRequest {
    /// Owned clipboard staging only; removed by the worker after original admission.
    pub temporary: Option<PathBuf>,
    pub paths: Vec<PathBuf>,
    pub embedded: bool,
    pub position: [f64; 2],
    pub sampling: ImageFiltering,
    pub mouse_easter_zoom: Option<f64>,
    pub work: PathBuf,
    pub spool: Option<Arc<File>>,
}
pub struct AdmittedImage {
    pub source: Source,
    pub asset: ImageAsset,
    pub object: DocumentObject,
    pub original: Option<Payload>,
    pub sampling: ImageFiltering,
}
pub enum ImportUpdate {
    Admitted(Box<AdmittedImage>),
    Progress {
        completed: usize,
        total: usize,
        bytes: u64,
    },
    Failed {
        path: PathBuf,
        message: String,
    },
    Finished {
        cancelled: bool,
        admitted: usize,
        bytes: u64,
        spool: Option<Arc<File>>,
    },
}
fn pin_copy(
    path: &Path,
    work: &Path,
    spool: &mut Option<Arc<File>>,
    cancel: &AtomicBool,
    bytes: &mut u64,
) -> Result<Payload, AssetError> {
    if spool.is_none() {
        let path = work.join("originals.spool");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        *spool = Some(Arc::new(options.open(path)?));
    }
    let file = spool.as_ref().ok_or("import spool unavailable")?;
    // A separately opened writer has an independent OS cursor. Windows seek_read
    // on the shared reader handle must never redirect append writes.
    let mut output = OpenOptions::new()
        .write(true)
        .open(work.join("originals.spool"))?;
    let offset = output.seek(SeekFrom::End(0))?;
    let mut input = File::open(path)?;
    let length = input.metadata()?.len();
    if length == 0
        || length > MAX_ENCODED_IMAGE
        || length > MAX_IMPORT_BYTES.saturating_sub(*bytes)
        || length > MAX_SESSION_IMPORT_BYTES.saturating_sub(offset)
    {
        return Err("embedded import exceeds 64 MiB/image, 2 GiB/batch or 4 GiB/session".into());
    }
    let copied = (|| -> Result<u32, AssetError> {
        let mut buffer = [0; 128 * 1024];
        let mut crc = crc32fast::Hasher::new();
        let mut count = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("import cancelled".into());
            }
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > length {
                return Err("source grew during import".into());
            }
            output.write_all(&buffer[..n])?;
            crc.update(&buffer[..n]);
        }
        if count != length {
            return Err("source shortened during import".into());
        }
        output.sync_all()?;
        Ok(crc.finalize())
    })();
    let crc32 = match copied {
        Ok(crc) => crc,
        Err(e) => {
            output.set_len(offset)?;
            return Err(e);
        }
    };
    *bytes += length;
    Ok(Payload::Stored {
        file: Arc::clone(file),
        range: BlobRange {
            offset,
            len: length,
            crc32,
        },
    })
}
impl ImportRequest {
    pub fn run(
        self,
        cancel: &AtomicBool,
        mut emit: impl FnMut(ImportUpdate) -> bool,
    ) -> Result<(), AssetError> {
        if self.paths.is_empty()
            || self.paths.len() > MAX_IMPORT_FILES
            || !self.position.into_iter().all(f64::is_finite)
        {
            return Err("import needs 1..4096 paths and a finite placement".into());
        }
        tack_storage::create_private_directory(&self.work, true)?;
        let mut spool = self.spool;
        let total = self.paths.len();
        let mut bytes = 0;
        let mut admitted = 0;
        let mut shared = BTreeMap::<PathBuf, (Source, ImageAsset)>::new();
        for (i, path) in self.paths.into_iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let result = (|| -> Result<AdmittedImage, AssetError> {
                let path = path.canonicalize()?;
                let fingerprint = source_fingerprint(&path)?;
                let (pixels, _) = image_metadata(&path)?;
                if fingerprint.size > MAX_ENCODED_IMAGE {
                    return Err("image exceeds 64 MiB encoded-input limit".into());
                }
                let (source, asset, original) = if let Some((s, a)) = shared.get(&path) {
                    (s.clone(), *a, None)
                } else {
                    let id = tack_storage::new_source_id()?;
                    let original = if self.embedded {
                        Some(pin_copy(&path, &self.work, &mut spool, cancel, &mut bytes)?)
                    } else {
                        None
                    };
                    if source_fingerprint(&path)? != fingerprint {
                        return Err("source changed during import".into());
                    }
                    let location = if self.embedded {
                        SourceLocation::Embedded
                    } else {
                        SourceLocation::Linked(LinkedPath::native(&path)?)
                    };
                    let source = Source::from_descriptor(id, location, 1, Some(fingerprint))?;
                    let asset = ImageAsset::new(tack_storage::new_asset_id()?, id, pixels)?;
                    shared.insert(path, (source.clone(), asset));
                    (source, asset, original)
                };
                let scale = (350. / f64::from(pixels[0])).min(300. / f64::from(pixels[1]));
                let size = pixels.map(|p| f64::from(p) * scale);
                let center = [
                    self.position[0] + (i % 8) as f64 * 400.,
                    self.position[1] + (i / 8) as f64 * 350.,
                ];
                let object = DocumentObject::image(
                    tack_storage::new_object_id()?,
                    asset.id(),
                    Transform::new(center, size, 0., [false; 2])?,
                );
                let mut image = AdmittedImage {
                    source,
                    asset,
                    object,
                    original,
                    sampling: self.sampling,
                };
                if let Some(zoom) = self.mouse_easter_zoom {
                    crate::mouse_tool::prepare_easter(&mut image, zoom)?;
                }
                Ok(image)
            })();
            match result {
                Ok(image) => {
                    if !emit(ImportUpdate::Admitted(Box::new(image))) {
                        return Ok(());
                    }
                    admitted += 1;
                }
                Err(_) if cancel.load(Ordering::Relaxed) => break,
                Err(error) => {
                    if !emit(ImportUpdate::Failed {
                        path,
                        message: error.to_string(),
                    }) {
                        return Ok(());
                    }
                }
            }
            if !emit(ImportUpdate::Progress {
                completed: i + 1,
                total,
                bytes,
            }) {
                return Ok(());
            }
        }
        emit(ImportUpdate::Finished {
            cancelled: cancel.load(Ordering::Relaxed),
            admitted,
            bytes,
            spool,
        });
        Ok(())
    }
}
pub fn admit(editor: &mut DocumentEditor, image: &AdmittedImage) -> Result<bool, AssetError> {
    let mut commands = Vec::with_capacity(4);
    if let Some(source) = editor.document().source(image.source.id()) {
        if source != &image.source
            && !(editor.is_shared() && source.revision() == image.source.revision())
        {
            return Err("import source identity changed before admission".into());
        }
    } else {
        commands.push(Command::AddSource(image.source.clone()));
    }
    if editor.document().asset(image.asset.id()).is_none() {
        commands.push(Command::AddAsset(image.asset));
    }
    commands.push(Command::AddObject {
        object: image.object.clone(),
        index: editor.document().object_order().len(),
    });
    commands.push(Command::SetImageFiltering {
        object: image.object.id(),
        filtering: image.sampling,
    });
    Ok(editor.execute(Command::Batch(commands))?)
}
