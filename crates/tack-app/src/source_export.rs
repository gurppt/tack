//! Explicit source retrieval: encoded bytes stream unchanged, with no decoder or idle worker.
use crate::{
    image_save::Originals,
    native_files::{self, PickOptions, Picker},
};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tack_assets::AssetError;
use tack_core::{Document, DocumentQuery, ObjectId, Source, SourceLocation};
use tack_storage::{Payload, RangeReader, TackFile};

pub struct OriginalExport {
    source: Source,
    board_path: PathBuf,
    payload: Option<Payload>,
}
impl OriginalExport {
    /// Only descriptor/handle lookup here. All stat, picker and copy work runs off-thread.
    pub fn new(
        doc: &Document,
        id: ObjectId,
        board_path: &Path,
        board: &TackFile,
        originals: &Originals,
    ) -> Result<Self, AssetError> {
        let source = doc
            .object_render_data(id)
            .and_then(|data| doc.asset(data.asset_id))
            .and_then(|asset| doc.source(asset.source_id()))
            .ok_or("Save Original As requires one selected image")?
            .clone();
        let payload = if matches!(source.location(), SourceLocation::Embedded) {
            Some(
                originals
                    .get(&(source.id(), source.revision()))
                    .cloned()
                    .or_else(|| {
                        board
                            .originals
                            .get(&source.id())
                            .filter(|entry| entry.revision == source.revision())
                            .map(|entry| board.payload(entry.range))
                    })
                    .ok_or("embedded original is unavailable")?,
            )
        } else {
            None
        };
        Ok(Self {
            source,
            board_path: board_path.to_owned(),
            payload,
        })
    }
    fn payload(&self) -> Result<Payload, AssetError> {
        if let Some(payload) = &self.payload {
            return Ok(payload.clone());
        }
        let SourceLocation::Linked(descriptor) = self.source.location() else {
            return Err("embedded original is unavailable".into());
        };
        let path = descriptor
            .to_native()
            .ok_or("source path belongs to another platform; relink first")?;
        let path = if path.is_absolute() {
            path
        } else {
            std::path::absolute(&self.board_path)?
                .parent()
                .ok_or("board directory unavailable")?
                .join(path)
        };
        let file = File::open(&path).map_err(|_| "linked original is missing or unavailable")?;
        if !file.metadata()?.is_file() {
            return Err("linked original is not a regular file".into());
        }
        // Keep a stable opened inode during picker/copy, without buffering its bytes.
        let len = file.metadata()?.len();
        Ok(Payload::Stored {
            file: Arc::new(file),
            range: tack_storage::BlobRange {
                offset: 0,
                len,
                crc32: 0,
            },
        })
    }
    fn suggested_name(&self, payload: &Payload) -> Result<OsString, AssetError> {
        if let SourceLocation::Linked(descriptor) = self.source.location()
            && let Some(path) = descriptor.to_native()
            && let Some(name) = path.file_name()
        {
            return Ok(name.to_os_string());
        }
        let mut header = [0u8; 8];
        let count = reader(payload)?.read(&mut header)?;
        let extension = if count == 8 && header == *b"\x89PNG\r\n\x1a\n" {
            "png"
        } else if count >= 3 && header[..3] == [255, 216, 255] {
            "jpg"
        } else {
            "bin"
        };
        Ok(format!("original-{:032x}.{extension}", self.source.id().value()).into())
    }
    pub fn pick_and_save(self, work: &Path, cancel: &AtomicBool) -> Result<(), AssetError> {
        let payload = self.payload()?;
        let options = PickOptions {
            directory: None,
            suggested_name: Some(self.suggested_name(&payload)?),
        };
        let paths =
            native_files::pick_with_options(Picker::ExportOriginal, work, cancel, &options)?;
        let path = paths.first().ok_or("export destination unavailable")?;
        if fs::canonicalize(path)
            .ok()
            .is_some_and(|path| fs::canonicalize(&self.board_path).ok().as_ref() == Some(&path))
        {
            return Err("export destination is the current board; choose another filename".into());
        }
        let expected_crc = matches!(self.source.location(), SourceLocation::Embedded);
        copy_atomic(&payload, path, cancel, expected_crc)
    }
}
fn reader(payload: &Payload) -> Result<Box<dyn Read>, AssetError> {
    Ok(match payload {
        Payload::File(path) => Box::new(File::open(path)?),
        Payload::Stored { file, range } => Box::new(RangeReader::new(Arc::clone(file), *range)),
    })
}
/// Existing regular files are replaced only after a complete, validated private sibling write.
/// Stored original CRC validation is mandatory; linked files have no stored checksum.
pub fn copy_atomic(
    payload: &Payload,
    path: &Path,
    cancel: &AtomicBool,
    validate_crc: bool,
) -> Result<(), AssetError> {
    if cancel.load(Ordering::Relaxed) {
        return Err("original export cancelled".into());
    }
    require_regular_target(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut leaf = path
        .file_name()
        .ok_or("export filename unavailable")?
        .to_os_string();
    leaf.push(".tack-export-lock");
    let _lock = tack_storage::lock_sidecar(&path.with_file_name(leaf))?;
    require_regular_target(path)?;
    let mut input = reader(payload)?;
    let length = payload.len()?;
    if length == 0 {
        return Err("original is empty or unavailable".into());
    }
    let temporary = parent.join(format!(
        ".tack-original-{:032x}.tmp",
        tack_storage::new_document_id()?.value()
    ));
    let result = (|| -> Result<(), AssetError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&temporary)?;
        let crc = copy_chunks(input.as_mut(), &mut output, length, cancel)?;
        if validate_crc
            && let Payload::Stored { range, .. } = payload
            && crc != range.crc32
        {
            return Err("embedded original checksum is corrupt; destination retained".into());
        }
        output.sync_all()?;
        drop(output);
        if cancel.load(Ordering::Relaxed) {
            return Err("original export cancelled".into());
        }
        require_regular_target(path)?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    result
}
fn copy_chunks(
    input: &mut dyn Read,
    output: &mut dyn Write,
    length: u64,
    cancel: &AtomicBool,
) -> Result<u32, AssetError> {
    let mut copied = 0u64;
    let mut crc = crc32fast::Hasher::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("original export cancelled".into());
        }
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        copied = copied
            .checked_add(count as u64)
            .ok_or("original length overflow")?;
        if copied > length {
            return Err("original changed while copying".into());
        }
        output.write_all(&buffer[..count])?;
        crc.update(&buffer[..count]);
    }
    if copied != length {
        return Err("original is truncated or changed while copying".into());
    }
    Ok(crc.finalize())
}
fn require_regular_target(path: &Path) -> Result<(), AssetError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Err("export destination must be a regular file".into())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests;
