use std::{fs, path::PathBuf, sync::atomic::AtomicBool};
use tack_app::source_transition::Request;
use tack_core::*;
use tack_storage::*;
type R<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Root(PathBuf);
impl Root {
    fn new() -> R<Self> {
        let p = std::env::temp_dir().join(format!(
            "tack-refresh-test-{:032x}",
            new_document_id()?.value()
        ));
        fs::create_dir(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 244, 34, 127, 138, 0, 0, 0, 14, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31,
    132, 1, 17, 247, 3, 253, 227, 197, 245, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
fn request(e: &DocumentEditor, r: &Root, toggle: bool) -> R<Request> {
    Ok(Request {
        document: e.document().id(),
        previous: e.document().sources().next().ok_or("source")?.clone(),
        board: r.0.join("board.tack"),
        work: r.0.join("work"),
        spool: None,
        toggle,
    })
}
fn fixture(r: &Root) -> R<DocumentEditor> {
    let path = r.0.join("source.png");
    fs::write(&path, PNG)?;
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        &path,
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [2, 1],
    )?))?;
    d.apply(Command::AddObject {
        object: DocumentObject::image(
            ObjectId::new(1)?,
            AssetId::new(1)?,
            Transform::new([10., 20.], [600., 300.], 0.3, [true, false])?,
        ),
        index: 0,
    })?;
    d.apply(Command::SetCrop {
        object: ObjectId::new(1)?,
        crop: Crop::new(0.1, 0.1, 0.8, 0.8)?,
    })?;
    Ok(DocumentEditor::new(d, 64))
}
#[test]
fn linked_embed_link_refresh_preserve_visuals_and_monotonic_invalidation_keys() -> R {
    let r = Root::new()?;
    let mut e = fixture(&r)?;
    let before = e.document().clone();
    let cancel = AtomicBool::new(false);
    let ready = request(&e, &r, true)?.run(&cancel)?.ok_or("ready")?;
    let (id, rev, original) = ready.apply(&mut e)?;
    assert!(rev > 1);
    assert!(original.is_some());
    assert_eq!(e.undo_len(), 1);
    let source = e.document().source(id).ok_or("source")?;
    assert!(matches!(source.location(), SourceLocation::Embedded));
    assert!(source.original_path().is_some());
    assert_eq!(
        e.document().object(ObjectId::new(1)?),
        before.object(ObjectId::new(1)?)
    );
    let (schema, counts, bytes) = encode_metadata(e.document())?;
    assert_eq!(schema, 8);
    assert_eq!(decode_metadata(schema, counts, &bytes)?, *e.document());
    let (id, second, no_bytes) = request(&e, &r, true)?
        .run(&cancel)?
        .ok_or("ready")?
        .apply(&mut e)?;
    assert!(second > rev);
    assert!(no_bytes.is_none());
    assert!(matches!(
        e.document().source(id).ok_or("source")?.location(),
        SourceLocation::Linked(_)
    ));
    let mut changed = PNG.to_vec();
    changed.extend_from_slice(b"new metadata after png");
    fs::write(r.0.join("source.png"), &changed)?;
    let (_, third, _) = request(&e, &r, false)?
        .run(&cancel)?
        .ok_or("ready")?
        .apply(&mut e)?;
    assert!(third > second);
    assert_eq!(
        e.document()
            .source(id)
            .ok_or("source")?
            .fingerprint()
            .ok_or("fp")?
            .size,
        changed.len() as u64
    );
    assert_eq!(
        e.document().object(ObjectId::new(1)?),
        before.object(ObjectId::new(1)?)
    );
    e.undo()?;
    e.undo()?;
    e.undo()?;
    assert_eq!(e.document(), &before);
    let (_, after_undo, _) = request(&e, &r, false)?
        .run(&cancel)?
        .ok_or("ready")?
        .apply(&mut e)?;
    assert!(after_undo > third); // Old LOD keys cannot be reused after an Undo/new refresh.
    Ok(())
}
#[test]
fn cancelled_and_stale_completions_do_not_mutate_document() -> R {
    let r = Root::new()?;
    let mut e = fixture(&r)?;
    let before = e.document().clone();
    assert!(
        request(&e, &r, true)?
            .run(&AtomicBool::new(true))?
            .is_none()
    );
    assert_eq!(e.document(), &before);
    let ready = request(&e, &r, false)?
        .run(&AtomicBool::new(false))?
        .ok_or("ready")?;
    e.execute(Command::SetSource(Source::from_descriptor(
        SourceId::new(1)?,
        SourceLocation::Embedded,
        7,
        None,
    )?))?;
    let changed = e.document().clone();
    assert!(ready.apply(&mut e).is_err());
    assert_eq!(e.document(), &changed);
    // An Embedded source without a remembered origin has no implicit refresh.
    assert!(
        request(&e, &r, false)?
            .run(&AtomicBool::new(false))
            .is_err()
    );
    Ok(())
}
