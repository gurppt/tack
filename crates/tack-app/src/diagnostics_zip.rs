//! Classic ZIP/STORE for bounded, already captured diagnostic files. No compression
//! library, shell, network, arbitrary directory recursion or permanent state.
use std::{
    fs::File,
    io::{self, Read, Seek, Write},
    path::{Component, Path, PathBuf},
};
const LIMIT: u64 = 100 * 1024 * 1024;
struct Entry {
    name: Vec<u8>,
    crc: u32,
    bytes: u32,
    offset: u32,
}
fn u16s(out: &mut File, values: &[u16]) -> io::Result<()> {
    for value in values {
        out.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}
fn u32s(out: &mut File, values: &[u32]) -> io::Result<()> {
    for value in values {
        out.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}
pub fn store(root: &Path, archive: &Path, files: &[PathBuf]) -> io::Result<()> {
    if files.len() > 256 {
        return Err(io::Error::other("Report file-count limit"));
    }
    let mut out = File::options().write(true).create_new(true).open(archive)?;
    let result = (|| {
        let mut entries = Vec::new();
        let mut total = 0u64;
        for relative in files {
            if relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            {
                return Err(io::Error::other("Unsafe report entry"));
            }
            let name = relative
                .components()
                .map(|c| {
                    c.as_os_str()
                        .to_str()
                        .ok_or_else(|| io::Error::other("Report entry encoding"))
                })
                .collect::<io::Result<Vec<_>>>()?
                .join("/")
                .into_bytes();
            if name.is_empty() || name.contains(&b'\\') || name.contains(&b':') || name.contains(&0)
            {
                return Err(io::Error::other("Unsafe report entry name"));
            }
            let length = u16::try_from(name.len()).map_err(io::Error::other)?;
            let path = root.join(relative);
            if !std::fs::symlink_metadata(&path)?.is_file() {
                return Err(io::Error::other("Report entry must be regular"));
            }
            let mut input = File::open(path)?;
            if input.metadata()?.len() > 4 * 1024 * 1024 {
                return Err(io::Error::other("Report entry size limit"));
            }
            let offset = u32::try_from(out.stream_position()?).map_err(io::Error::other)?;
            // UTF-8, streamed descriptor, STORE (method 0), deterministic 1980-01-01.
            u32s(&mut out, &[0x04034b50])?;
            u16s(&mut out, &[20, 0x0808, 0, 0, 33])?;
            u32s(&mut out, &[0, 0, 0])?;
            u16s(&mut out, &[length, 0])?;
            out.write_all(&name)?;
            let mut crc = crc32fast::Hasher::new();
            let mut bytes = 0u32;
            let mut buffer = [0; 64 * 1024];
            loop {
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                bytes = bytes
                    .checked_add(count as u32)
                    .ok_or_else(|| io::Error::other("Report size overflow"))?;
                total += count as u64;
                if bytes > 4 * 1024 * 1024 || total > LIMIT {
                    return Err(io::Error::other("Report size limit"));
                }
                out.write_all(&buffer[..count])?;
                crc.update(&buffer[..count]);
            }
            let crc = crc.finalize();
            u32s(&mut out, &[0x08074b50, crc, bytes, bytes])?;
            entries.push(Entry {
                name,
                crc,
                bytes,
                offset,
            });
        }
        let start = u32::try_from(out.stream_position()?).map_err(io::Error::other)?;
        for e in &entries {
            u32s(&mut out, &[0x02014b50])?;
            u16s(&mut out, &[20, 20, 0x0808, 0, 0, 33])?;
            u32s(&mut out, &[e.crc, e.bytes, e.bytes])?;
            u16s(&mut out, &[e.name.len() as u16, 0, 0, 0, 0])?;
            u32s(&mut out, &[0, e.offset])?;
            out.write_all(&e.name)?;
        }
        let size =
            u32::try_from(out.stream_position()? - u64::from(start)).map_err(io::Error::other)?;
        u32s(&mut out, &[0x06054b50])?;
        u16s(
            &mut out,
            &[0, 0, entries.len() as u16, entries.len() as u16],
        )?;
        u32s(&mut out, &[size, start])?;
        u16s(&mut out, &[0])?;
        out.sync_all()
    })();
    drop(out);
    if result.is_err() {
        let _ = std::fs::remove_file(archive);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_existing_archive_and_unsafe_entries_without_clobbering() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "tack-report-store-{}",
            tack_storage::new_document_id()
                .map_err(io::Error::other)?
                .value()
        ));
        std::fs::create_dir(&root)?;
        let archive = root.join("report.zip");
        std::fs::write(&archive, b"previous report")?;
        assert!(store(&root, &archive, &[]).is_err());
        assert_eq!(std::fs::read(&archive)?, b"previous report");
        let bad = root.join("bad.zip");
        assert!(store(&root, &bad, &["../private".into()]).is_err());
        assert!(!bad.exists());
        std::fs::remove_dir_all(root)
    }
}
