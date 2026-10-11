//! Finite streaming source export. New destination tree; existing artwork is never replaced.
use crate::{
    image_save::Originals,
    native_files::{self, Picker},
    source_export::{OriginalExport, copy_atomic},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tack_assets::AssetError;
use tack_core::{AnnotationKind, Document, DocumentQuery, ObjectKind, SourceLocation};
use tack_storage::TackFile;
pub const MAX_ITEMS: usize = 4096;
pub struct Request {
    images: Vec<(String, OriginalExport)>,
    notes: BTreeMap<String, String>,
}
impl Request {
    pub fn new(
        doc: &Document,
        selection: Option<&crate::image_interaction::SelectionState>,
        board_path: &Path,
        board: &TackFile,
        originals: &Originals,
        by_frame: bool,
    ) -> Result<Self, AssetError> {
        if doc.frame_count() > MAX_ITEMS {
            return Err("Source export exceeds 4096 Frames".into());
        }
        let mut frames = BTreeMap::new();
        let mut names = BTreeSet::from(["_unframed".to_string()]);
        for id in doc.object_order() {
            if let Some(o) = doc.object(*id)
                && let ObjectKind::Frame(name) = o.kind()
            {
                frames.insert(*id, unique(&sanitize(name), "", &mut names));
            }
        }
        let mut request = Self {
            images: Vec::new(),
            notes: BTreeMap::new(),
        };
        let mut count = 0;
        let mut note_bytes = 0usize;
        for id in doc.object_order() {
            let Some(o) = doc.object(*id) else {
                continue;
            };
            let parent = doc.frame_parent(*id).or_else(|| {
                frames.keys().copied().find(|f| {
                    doc.object(*f).is_some_and(|f| {
                        crate::image_geometry::hit(f.transform(), o.transform().center())
                    })
                })
            });
            if selection.is_some_and(|s| !s.contains(*id) && !parent.is_some_and(|f| s.contains(f)))
            {
                continue;
            }
            let folder = if by_frame {
                parent
                    .and_then(|f| frames.get(&f))
                    .map_or("_unframed", String::as_str)
            } else {
                ""
            }
            .to_owned();
            if doc.object_render_data(*id).is_some() {
                request.images.push((
                    folder,
                    OriginalExport::new(doc, *id, board_path, board, originals)?,
                ));
                count += 1;
            } else if let ObjectKind::Annotation(a) = o.kind()
                && let AnnotationKind::Text(text) = a.kind()
            {
                let notes = request.notes.entry(folder).or_default();
                if notes.len().saturating_add(text.value().len() + 2) > 1024 * 1024 {
                    return Err("Export notes exceed 1 MiB per folder".into());
                }
                note_bytes = note_bytes.saturating_add(text.value().len() + 2);
                if note_bytes > 8 * 1024 * 1024 {
                    return Err("Export notes exceed 8 MiB total; select a smaller set".into());
                }
                notes.push_str(text.value());
                notes.push_str("\n\n");
                count += 1;
            }
            if count > MAX_ITEMS {
                return Err("Source export exceeds 4096 items; select a smaller set".into());
            }
        }
        if count == 0 {
            return Err("No image sources or Text Blocks to export".into());
        }
        Ok(request)
    }
    pub fn pick_and_save(self, work: &Path, cancel: &AtomicBool) -> Result<(), AssetError> {
        let paths = native_files::pick(Picker::ExportSources, work, cancel)?;
        if let Some(path) = paths.first() {
            self.export_to(path, cancel)?;
        }
        Ok(())
    }
    pub fn export_to(&self, parent: &Path, cancel: &AtomicBool) -> Result<PathBuf, AssetError> {
        if !fs::symlink_metadata(parent)
            .map_err(|e| format!("Export directory {} is unavailable: {e}", parent.display()))?
            .is_dir()
        {
            return Err("Export destination must be a directory".into());
        }
        let root = new_root(parent)?;
        // Only known, newly created directories are written; cancellation leaves an explicit partial tree.
        let mut names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (folder, _) in &self.images {
            names
                .entry(folder.clone())
                .or_default()
                .insert("notes.txt".into());
        }
        for (folder, original) in &self.images {
            if cancel.load(Ordering::Relaxed) {
                return Err("Source export cancelled; partial Tack Sources folder retained".into());
            }
            let payload = original.payload()?;
            let hint = original
                .source
                .original_path()
                .and_then(|p| p.to_native())
                .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()));
            let suggested = original.suggested_name(&payload)?;
            let suggested = Path::new(&suggested);
            let origin = original.source.original_path().and_then(|p| p.to_native());
            let safe_ext = |p: &Path| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .filter(|s| {
                        matches!(
                            s.to_ascii_lowercase().as_str(),
                            "png" | "jpg" | "jpeg" | "bin"
                        )
                    })
                    .map(str::to_owned)
            };
            let extension = origin
                .as_deref()
                .and_then(safe_ext)
                .or_else(|| safe_ext(suggested))
                .unwrap_or_else(|| "bin".into());
            let stem = sanitize(hint.as_deref().unwrap_or("image"));
            let name = unique(&stem, &extension, names.entry(folder.clone()).or_default());
            let destination = directory(&root, folder)?.join(name);
            copy_atomic(
                &payload,
                &destination,
                cancel,
                matches!(original.source.location(), SourceLocation::Embedded),
            )?;
        }
        for (folder, notes) in &self.notes {
            if cancel.load(Ordering::Relaxed) {
                return Err("Source export cancelled; partial Tack Sources folder retained".into());
            }
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory(&root, folder)?.join("notes.txt"))?;
            use std::io::Write;
            output.write_all(notes.as_bytes())?;
            output.sync_all()?;
        }
        Ok(root)
    }
}
fn new_root(parent: &Path) -> Result<PathBuf, AssetError> {
    for n in 1..=10_000 {
        let root = parent.join(if n == 1 {
            "Tack Sources".into()
        } else {
            format!("Tack Sources_{n}")
        });
        match fs::create_dir(&root) {
            Ok(()) => return Ok(root),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    Err("Too many existing Tack Sources folders".into())
}
fn directory(root: &Path, folder: &str) -> Result<PathBuf, AssetError> {
    if folder.is_empty() {
        return Ok(root.into());
    }
    let path = root.join(folder);
    match fs::create_dir(&path) {
        Ok(()) => {}
        Err(e)
            if e.kind() == std::io::ErrorKind::AlreadyExists
                && fs::symlink_metadata(&path)?.is_dir() => {}
        Err(e) => return Err(e.into()),
    }
    Ok(path)
}
pub(crate) fn sanitize(name: &str) -> String {
    let mut s: String = name
        .chars()
        .take(80)
        .scan(0usize, |bytes, c| {
            *bytes += c.len_utf8();
            (*bytes <= 160).then_some(c)
        })
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    s = s
        .trim_matches(|c: char| c.is_whitespace() || c == '.')
        .to_owned();
    let base = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    if s.is_empty() {
        s = "Untitled".into();
    }
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .contains(&base.as_str())
    {
        s.insert(0, '_');
    }
    s
}
fn unique(stem: &str, extension: &str, used: &mut BTreeSet<String>) -> String {
    let ext = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{stem}{ext}")
        } else {
            format!("{stem}_{n}{ext}")
        };
        if used.insert(name.to_lowercase()) {
            return name;
        }
        n += 1;
    }
}
#[cfg(test)]
mod tests;
