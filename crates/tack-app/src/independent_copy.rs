//! Explicit worker-only local fork. Asset bytes stream once into persistent companions.
use crate::local_worker::OpenedBoard;
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tack_assets::AssetError;
use tack_core::{Command, Document, LinkedPath, Source, SourceLocation};
use tack_shared::{SourceBinding, hash_reader};
use tack_storage::TackFile;
pub struct Request {
    pub document: Document,
    pub board: Arc<TackFile>,
    pub board_path: PathBuf,
    pub sources: Vec<SourceBinding>,
    pub cache: Option<PathBuf>,
    pub server: Option<String>,
    pub target: PathBuf,
}
impl Request {
    pub fn run(self, cancel: &AtomicBool) -> Result<OpenedBoard, AssetError> {
        let target = std::path::absolute(crate::file_names::board(&self.target))?;
        if fs::symlink_metadata(&target).is_ok() || crate::sharing::sidecar(&target).exists() {
            return Err("Choose a new local filename".into());
        }
        let mut folder_name = target.file_name().ok_or("local filename")?.to_os_string();
        folder_name.push(".assets");
        let folder = target.with_file_name(&folder_name);
        let mut document = self.document.fork(tack_storage::new_document_id()?);
        let sources: Vec<_> = document.sources().cloned().collect();
        if !sources.is_empty() {
            tack_storage::create_private_directory(&folder, false)?;
        }
        let result = (|| -> Result<OpenedBoard, AssetError> {
            for source in sources {
                check_cancel(cancel)?;
                let binding = self.sources.iter().find(|b| {
                    b.source.value() == source.id().value() && b.revision == source.revision()
                });
                let name = binding.map_or_else(
                    || format!("{:032x}", source.id().value()),
                    |b| b.hash.to_string(),
                );
                let path = folder.join(&name);
                if !path.exists() {
                    self.copy_source(&source, binding, &path, cancel)?;
                }
                let relative = PathBuf::from(&folder_name).join(name);
                document.apply(Command::SetSource(Source::from_descriptor(
                    source.id(),
                    SourceLocation::Linked(LinkedPath::native(&relative)?),
                    source
                        .revision()
                        .checked_add(1)
                        .ok_or("source revision exhausted")?,
                    None,
                )?))?;
            }
            #[cfg(unix)]
            if folder.exists() {
                fs::File::open(&folder)?.sync_all()?;
            }
            check_cancel(cancel)?;
            let owner = tack_storage::BoardLease::acquire_new(&target)?;
            owner.save(&document, vec![])?;
            drop(owner);
            OpenedBoard::read(&target)
        })();
        // A post-publication durability error must never strand a published board.
        if result.is_err() && !target.exists() && folder.exists() {
            let _ = fs::remove_dir_all(&folder);
        }
        result
    }
    fn copy_source(
        &self,
        source: &Source,
        binding: Option<&SourceBinding>,
        target: &std::path::Path,
        cancel: &AtomicBool,
    ) -> Result<(), AssetError> {
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        if let Some(entry) = self
            .board
            .originals
            .get(&source.id())
            .filter(|o| o.revision == source.revision())
        {
            let mut reader = self.board.original_reader(source.id())?;
            let (size, crc) = copy(&mut reader, &mut output, cancel)?;
            if size != entry.range.len || crc != entry.range.crc32 {
                return Err("Local original checksum mismatch; shared board retained".into());
            }
        } else if let Some(binding) = binding {
            let cached = self
                .cache
                .as_ref()
                .map(|root| root.join(binding.hash.as_str()));
            if let Some(cached) = cached.filter(|p| p.is_file()) {
                copy(&mut fs::File::open(cached)?, &mut output, cancel)?;
            } else {
                let server = self
                    .server
                    .as_ref()
                    .ok_or("Original unavailable offline; reconnect before Save to Local")?;
                let mut stream = tack_shared::publish::connect(server, true)?;
                let mut offset = 0;
                while offset < binding.size {
                    check_cancel(cancel)?;
                    offset += tack_shared::publish::download_chunk(
                        &mut stream,
                        binding,
                        offset,
                        &mut output,
                    )? as u64;
                }
            }
        } else if let SourceLocation::Linked(descriptor) = source.location() {
            let native = descriptor
                .to_native()
                .ok_or("Source belongs to another platform")?;
            let path = if native.is_absolute() {
                native
            } else {
                self.board_path.parent().ok_or("board folder")?.join(native)
            };
            let before = tack_assets::source_fingerprint(&path)?;
            if source
                .fingerprint()
                .is_some_and(|expected| expected != before)
            {
                return Err("Linked source changed; shared board retained".into());
            }
            let mut file = fs::File::open(&path)?;
            let (size, _) = copy(&mut file, &mut output, cancel)?;
            if size != before.size || before != tack_assets::source_fingerprint(&path)? {
                return Err("Linked source changed while copying; shared board retained".into());
            }
        } else {
            return Err("Original unavailable offline; reconnect before Save to Local".into());
        }
        output.sync_all()?;
        drop(output);
        if let Some(binding) = binding {
            let (hash, size) = hash_reader(&mut fs::File::open(target)?)?;
            if hash != binding.hash || size != binding.size {
                return Err("Local original hash mismatch; shared board retained".into());
            }
        }
        Ok(())
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), AssetError> {
    if cancel.load(Ordering::Relaxed) {
        Err("Save to Local cancelled; shared board retained".into())
    } else {
        Ok(())
    }
}
fn copy(
    reader: &mut impl Read,
    output: &mut impl Write,
    cancel: &AtomicBool,
) -> Result<(u64, u32), AssetError> {
    let mut bytes = [0; 65536];
    let mut size = 0u64;
    let mut crc = crc32fast::Hasher::new();
    loop {
        check_cancel(cancel)?;
        let n = reader.read(&mut bytes)?;
        if n == 0 {
            return Ok((size, crc.finalize()));
        }
        size += n as u64;
        crc.update(&bytes[..n]);
        output.write_all(&bytes[..n])?;
    }
}
