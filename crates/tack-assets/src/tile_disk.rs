//! Worker-owned, disposable raw tile reuse. Originals always remain authoritative.
//!
//! One packed file has at most 251 fixed slots at the 64 MiB ceiling. Startup
//! reads only their 128-byte headers; tile bytes are checked lazily on a hit.
//! File I/O, lock acquisition and this compact LRU belong only on image workers.
use crate::{AssetError, Decoded};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const MAX_TILE_EDGE: u32 = 258;
pub const MAX_BUDGET: usize = 64 * 1024 * 1024;
const HEADER_BYTES: usize = 128;
const MAX_PAYLOAD: usize = MAX_TILE_EDGE as usize * MAX_TILE_EDGE as usize * 4;
const SLOT_BYTES: usize = HEADER_BYTES + MAX_PAYLOAD;
const FILE_HEADER_BYTES: usize = 16;
const FILE_MAGIC: &[u8; 8] = b"TACKR001";
const MAGIC: &[u8; 8] = b"TACKRAW1";
const FILE_NAME: &str = "tile-detail-v1.raw";
const LOCK_NAME: &str = "tile-detail-v1.lock";

#[derive(Clone, Copy)]
struct Entry {
    key: [u8; 80],
    width: u32,
    height: u32,
    bytes: usize,
    crc: u32,
    stamp: u64,
}

pub struct TileDisk {
    path: PathBuf,
    file: File,
    lock: File,
    slots: Vec<Option<Entry>>,
    capacity: usize,
    budget: usize,
    stamp: u64,
}

impl TileDisk {
    /// The caller allocates this quota from the *combined* derived disk budget.
    /// An unavailable/locked/unsafe cache returns an error; source decode remains
    /// usable. No directory enumeration or eagerly reserved payload file exists.
    /// Quotas smaller than the 16-byte global header disable this cache.
    pub fn open(root: &Path, budget: usize) -> Result<Self, AssetError> {
        let budget = budget.min(MAX_BUDGET);
        if budget < FILE_HEADER_BYTES {
            return Err("raw tile cache quota cannot hold its file header".into());
        }
        check_ancestry(root)?;
        tack_storage::create_private_directory(root, true)?;
        check_private_path(root, true)?;
        let lock_path = root.join(LOCK_NAME);
        check_optional_file(&lock_path)?;
        let lock = tack_storage::lock_sidecar(&lock_path)?;
        let path = root.join(FILE_NAME);
        check_optional_file(&path)?;
        let file = open_cache_file(&path)?;
        check_private_path(&path, false)?;
        if !file.metadata()?.is_file() {
            return Err("raw tile cache must be a regular file".into());
        }
        let capacity = (budget - FILE_HEADER_BYTES) / SLOT_BYTES;
        // A smaller quota and an interrupted final append discard only cache
        // bytes. Slot positions do not depend on the current quota.
        let payload_length = file
            .metadata()?
            .len()
            .checked_sub(FILE_HEADER_BYTES as u64)
            .ok_or("raw tile cache header truncated while opening")?;
        let count = (payload_length / SLOT_BYTES as u64).min(capacity as u64);
        file.set_len(FILE_HEADER_BYTES as u64 + count * SLOT_BYTES as u64)?;
        let mut disk = Self {
            path,
            file,
            lock,
            slots: vec![None; count as usize],
            capacity,
            budget,
            stamp: 0,
        };
        disk.load_headers()?;
        Ok(disk)
    }

    #[cfg(test)]
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Logical allocated file length, including slot headers and unused padding.
    pub fn used_bytes(&self) -> usize {
        (FILE_HEADER_BYTES + self.slots.len() * SLOT_BYTES).min(self.budget)
    }

    #[cfg(test)]
    pub fn payload_bytes(&self) -> usize {
        self.slots.iter().flatten().map(|entry| entry.bytes).sum()
    }

    #[cfg(test)]
    pub fn entries(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn get(&mut self, key: [u8; 80]) -> Result<Option<Decoded>, AssetError> {
        self.ensure_present()?;
        let Some(index) = self.find(key) else {
            return Ok(None);
        };
        let Some(mut entry) = self.read_header(index)? else {
            self.invalidate(index)?;
            return Ok(None);
        };
        if entry.key != key {
            self.invalidate(index)?;
            return Ok(None);
        }
        let mut rgba = vec![0; entry.bytes];
        self.file.read_exact(&mut rgba)?;
        if crc32fast::hash(&rgba) != entry.crc {
            self.invalidate(index)?;
            return Ok(None);
        }
        entry.stamp = self.next_stamp();
        self.write_header(index, entry)?;
        self.slots[index] = Some(entry);
        Ok(Some(Decoded {
            width: entry.width,
            height: entry.height,
            rgba,
        }))
    }

    /// At most one slot write. A rejected size/quota leaves existing cache data
    /// alone. Cache write errors must not discard successfully decoded pixels.
    pub fn put(&mut self, key: [u8; 80], image: &Decoded) -> Result<bool, AssetError> {
        let Some(bytes) = payload_size(image.width, image.height) else {
            return Ok(false);
        };
        if bytes != image.rgba.len() || self.capacity == 0 {
            return Ok(false);
        }
        self.ensure_present()?;
        let index = self.choose_slot(key);
        if index == self.slots.len() {
            // Grow only for visited tiles. This reserves one bounded slot's
            // padding, never the complete quota or a source-sized pyramid.
            self.file
                .set_len((FILE_HEADER_BYTES + (index + 1) * SLOT_BYTES) as u64)?;
            self.slots.push(None);
        }
        // Commit the checked header last. Partial overwrites cannot publish old
        // identity with new bytes, and torn payload writes fail their checksum.
        self.invalidate(index)?;
        self.file
            .seek(SeekFrom::Start(slot_offset(index) + HEADER_BYTES as u64))?;
        self.file.write_all(&image.rgba)?;
        let entry = Entry {
            key,
            width: image.width,
            height: image.height,
            bytes,
            crc: crc32fast::hash(&image.rgba),
            stamp: self.next_stamp(),
        };
        self.write_header(index, entry)?;
        self.slots[index] = Some(entry);
        Ok(true)
    }

    fn load_headers(&mut self) -> Result<(), AssetError> {
        for index in 0..self.slots.len() {
            let Some(entry) = self.read_header(index)? else {
                continue;
            };
            self.stamp = self.stamp.max(entry.stamp);
            // A malformed/interrupted cache must not accumulate duplicate keys.
            if let Some(prior) = self.find(entry.key) {
                if self.slots[prior].is_some_and(|old| old.stamp >= entry.stamp) {
                    continue;
                }
                self.slots[prior] = None;
            }
            self.slots[index] = Some(entry);
        }
        Ok(())
    }

    fn find(&self, key: [u8; 80]) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.is_some_and(|entry| entry.key == key))
    }

    fn choose_slot(&self, key: [u8; 80]) -> usize {
        if let Some(index) = self.find(key) {
            return index;
        }
        if let Some(index) = self.slots.iter().position(Option::is_none) {
            return index;
        }
        if self.slots.len() < self.capacity {
            return self.slots.len();
        }
        self.slots
            .iter()
            .enumerate()
            .min_by_key(|(_, slot)| slot.map_or(0, |entry| entry.stamp))
            .map_or(0, |(index, _)| index)
    }

    fn next_stamp(&mut self) -> u64 {
        self.stamp = self.stamp.saturating_add(1);
        self.stamp
    }

    fn read_header(&mut self, index: usize) -> Result<Option<Entry>, AssetError> {
        let mut header = [0; HEADER_BYTES];
        self.file.seek(SeekFrom::Start(slot_offset(index)))?;
        self.file.read_exact(&mut header)?;
        Ok(decode_header(&header))
    }

    fn write_header(&mut self, index: usize, entry: Entry) -> Result<(), AssetError> {
        self.file.seek(SeekFrom::Start(slot_offset(index)))?;
        self.file.write_all(&encode_header(entry))?;
        Ok(())
    }

    fn invalidate(&mut self, index: usize) -> Result<(), AssetError> {
        self.slots[index] = None;
        self.file.seek(SeekFrom::Start(slot_offset(index)))?;
        self.file.write_all(&[0; 8])?;
        Ok(())
    }

    fn ensure_present(&mut self) -> Result<(), AssetError> {
        let metadata = std::fs::symlink_metadata(&self.path)?;
        let lock_metadata = std::fs::symlink_metadata(self.path.with_file_name(LOCK_NAME))?;
        if !metadata.is_file() || metadata.len() != self.used_bytes() as u64 {
            return Err("raw tile cache deleted, resized or replaced".into());
        }
        if !lock_metadata.is_file() {
            return Err("raw tile cache lock deleted or replaced".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let owned = self.file.metadata()?;
            if metadata.nlink() != 1 || lock_metadata.nlink() != 1 {
                return Err("raw tile cache gained an external hard link".into());
            }
            if (owned.dev(), owned.ino()) != (metadata.dev(), metadata.ino()) {
                return Err("raw tile cache replaced".into());
            }
            let lock_owned = self.lock.metadata()?;
            if (lock_owned.dev(), lock_owned.ino()) != (lock_metadata.dev(), lock_metadata.ino()) {
                return Err("raw tile cache lock replaced".into());
            }
        }
        check_file_header(&mut self.file)
    }
}

impl Drop for TileDisk {
    fn drop(&mut self) {
        // A decoder child can briefly inherit pre-exec handles; release the
        // cooperative lease explicitly, following storage's BoardLease policy.
        let _ = self.lock.unlock();
    }
}

fn slot_offset(index: usize) -> u64 {
    (FILE_HEADER_BYTES + index * SLOT_BYTES) as u64
}

fn file_header() -> [u8; FILE_HEADER_BYTES] {
    let mut header = [0; FILE_HEADER_BYTES];
    header[..8].copy_from_slice(FILE_MAGIC);
    header[8..12].copy_from_slice(&(SLOT_BYTES as u32).to_le_bytes());
    let crc = crc32fast::hash(&header[..12]);
    header[12..].copy_from_slice(&crc.to_le_bytes());
    header
}

fn check_file_header(file: &mut File) -> Result<(), AssetError> {
    let mut header = [0; FILE_HEADER_BYTES];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header)?;
    if header != file_header() {
        return Err("raw tile cache global header is invalid".into());
    }
    Ok(())
}

fn open_cache_file(path: &Path) -> Result<File, AssetError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(&file_header())?;
            Ok(file)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut file = options
                .create_new(false)
                .create(false)
                .truncate(false)
                .open(path)?;
            // Existing bytes are never changed until this distinct cache-only
            // prefix is checked, including where stable nlink is unavailable.
            check_file_header(&mut file)?;
            Ok(file)
        }
        Err(error) => Err(error.into()),
    }
}

fn payload_size(width: u32, height: u32) -> Option<usize> {
    if width == 0 || height == 0 || width > MAX_TILE_EDGE || height > MAX_TILE_EDGE {
        return None;
    }
    Some(width as usize * height as usize * 4)
}

fn encode_header(entry: Entry) -> [u8; HEADER_BYTES] {
    let mut header = [0; HEADER_BYTES];
    header[..8].copy_from_slice(MAGIC);
    header[8..88].copy_from_slice(&entry.key);
    header[88..92].copy_from_slice(&entry.width.to_le_bytes());
    header[92..96].copy_from_slice(&entry.height.to_le_bytes());
    header[96..100].copy_from_slice(&(entry.bytes as u32).to_le_bytes());
    header[100..104].copy_from_slice(&entry.crc.to_le_bytes());
    header[104..112].copy_from_slice(&entry.stamp.to_le_bytes());
    let crc = crc32fast::hash(&header[..124]);
    header[124..128].copy_from_slice(&crc.to_le_bytes());
    header
}

fn decode_header(header: &[u8; HEADER_BYTES]) -> Option<Entry> {
    let u32_at = |offset| {
        Some(u32::from_le_bytes(
            header.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    if &header[..8] != MAGIC
        || header[112..124] != [0; 12]
        || crc32fast::hash(&header[..124]) != u32_at(124)?
    {
        return None;
    }
    let width = u32_at(88)?;
    let height = u32_at(92)?;
    let bytes = payload_size(width, height)?;
    if bytes != u32_at(96)? as usize {
        return None;
    }
    Some(Entry {
        key: header[8..88].try_into().ok()?,
        width,
        height,
        bytes,
        crc: u32_at(100)?,
        stamp: u64::from_le_bytes(header[104..112].try_into().ok()?),
    })
}

fn check_ancestry(root: &Path) -> Result<(), AssetError> {
    for parent in root.ancestors().filter(|path| !path.as_os_str().is_empty()) {
        match std::fs::symlink_metadata(parent) {
            Ok(metadata) if !metadata.is_dir() => return Err("unsafe tile cache directory".into()),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn check_private_path(path: &Path, directory: bool) -> Result<(), AssetError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err("unsafe tile cache path".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if !directory && metadata.nlink() != 1 {
            return Err("tile cache files must not alias another file through hard links".into());
        }
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err("tile cache path must not be writable by other users".into());
        }
    }
    Ok(())
}

fn check_optional_file(path: &Path) -> Result<(), AssetError> {
    match check_private_path(path, false) {
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(())
        }
        result => result,
    }
}

#[cfg(test)]
#[path = "tile_disk/tests.rs"]
mod tests;
