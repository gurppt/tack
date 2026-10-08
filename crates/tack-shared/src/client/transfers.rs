use super::*;
use crate::cache::OriginalCache;
use crate::{ContentHash, encode_hex};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, io::Write};

pub(super) fn run(context: Context, requests: Receiver<Transfer>, control: SyncSender<Request>) {
    let mut cache = None;
    while let Ok(request) = requests.recv() {
        if context.cancel.load(Ordering::Relaxed) {
            break;
        }
        match request {
            Transfer::Asset(binding) => {
                let result = (|| -> Result<PathBuf> {
                    if cache.is_none() {
                        cache = Some(OriginalCache::open(context.config.cache_dir.clone())?);
                    }
                    download(
                        &context,
                        cache
                            .as_mut()
                            .ok_or(Error::Invalid("client cache unavailable"))?,
                        &binding,
                    )
                })();
                if let Some(cache) = cache.as_mut() {
                    let hashes = cache.take_evicted();
                    if !hashes.is_empty() {
                        context.emit(ClientEvent::AssetEvicted { hashes });
                    }
                }
                match result {
                    Ok(path) => {
                        context.emit(ClientEvent::AssetReady { binding, path });
                    }
                    Err(error) => {
                        context.emit(ClientEvent::AssetFailed {
                            binding,
                            reason: error.to_string(),
                        });
                    }
                }
            }
            Transfer::Prepare {
                epoch,
                operation,
                base,
                command,
                originals,
                path,
            } => {
                let result = (|| -> Result<()> {
                    if !context.connected.load(Ordering::Acquire)
                        || epoch != context.epoch.load(Ordering::Acquire)
                    {
                        return Err(Error::Invalid(
                            "edit preparation refused while disconnected",
                        ));
                    }
                    let (command, prepared) = crate::publish::prepare_command(
                        &command,
                        &originals,
                        &path,
                        &context.cancel,
                    )?;
                    let mut stream = crate::publish::connect(&context.config.address, true)?;
                    let mut stream = stats::Observed {
                        socket: &mut stream,
                        counters: Arc::clone(&context.counters),
                    };
                    let mut uploaded = HashSet::new();
                    for source in &prepared {
                        if uploaded.insert(source.binding.hash.clone()) {
                            crate::publish::upload_source(&mut stream, source, &context.cancel)?;
                        }
                    }
                    crate::publish::check_cancel(&context.cancel)?;
                    if !context.connected.load(Ordering::Acquire)
                        || epoch != context.epoch.load(Ordering::Acquire)
                    {
                        return Err(Error::Invalid(
                            "connection lost during source upload; edit not replayed",
                        ));
                    }
                    bounded_send(
                        &control,
                        Request::Edit {
                            epoch,
                            operation,
                            base,
                            command,
                            sources: prepared.into_iter().map(|s| s.binding).collect(),
                        },
                    )
                })();
                if let Err(error) = result {
                    context.emit(ClientEvent::Refused {
                        operation: Some(operation),
                        revision: base,
                        reason: error.to_string(),
                    });
                }
            }
        }
    }
}
fn download(
    context: &Context,
    cache: &mut OriginalCache,
    binding: &SourceBinding,
) -> Result<PathBuf> {
    binding.validate()?;
    crate::publish::check_cancel(&context.cancel)?;
    if let Some(path) = cache.available(&binding.hash, binding.size, &context.cancel)? {
        return Ok(path);
    }
    let (temporary, mut output) = cache.begin(binding.size)?;
    let result = (|| -> Result<PathBuf> {
        let mut stream = crate::publish::connect(&context.config.address, true)?;
        let mut stream = stats::Observed {
            socket: &mut stream,
            counters: Arc::clone(&context.counters),
        };
        let mut digest = Sha256::new();
        let mut offset = 0u64;
        while offset < binding.size {
            crate::publish::check_cancel(&context.cancel)?;
            let mut writer = HashWriter {
                file: &mut output,
                digest: &mut digest,
            };
            let count = crate::publish::download_chunk(&mut stream, binding, offset, &mut writer)?;
            offset = offset
                .checked_add(count as u64)
                .ok_or(Error::Invalid("download offset overflow"))?;
        }
        let hash = ContentHash::parse(&encode_hex(&digest.finalize()))?;
        if hash != binding.hash {
            return Err(Error::Invalid("download original hash mismatch"));
        }
        output.sync_all()?;
        drop(output);
        crate::publish::check_cancel(&context.cancel)?;
        cache.finish(&temporary, &binding.hash, binding.size)
    })();
    let _ = fs::remove_file(temporary);
    result
}
struct HashWriter<'a> {
    file: &'a mut fs::File,
    digest: &'a mut Sha256,
}
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.file.write(bytes)?;
        self.digest.update(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
