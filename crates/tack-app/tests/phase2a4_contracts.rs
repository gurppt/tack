//! Independent bounded contracts; no subprocess, GPU, network or corpus fixture.
use std::{fs, path::PathBuf};
use tack_app::{
    actions::Action,
    preferences::{self, Preferences},
    sharing::{self, Descriptor},
    toolbar::{self, Config, Placement, Toolbar},
    toolbar_icons,
};
use tack_core::{Command, Document, DocumentEditor, DocumentId, DocumentLimits, Source, SourceId};

type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Temp(PathBuf);
impl Temp {
    fn new() -> R<Self> {
        let path = std::env::temp_dir().join(format!(
            "tack-2a4-contract-{}",
            tack_storage::new_document_id()?.value()
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn maximum_toolbar_fits_small_client_and_hits_semantic_actions() -> R {
    let mut config = Config {
        actions: Action::ALL
            .into_iter()
            .filter(|a| toolbar::eligible(*a))
            .take(toolbar::MAX_ENTRIES)
            .map(Action::id)
            .collect(),
        offset: 8192,
        floating: [8192; 2],
        ..Default::default()
    };
    assert_eq!(config.actions.len(), 32);
    config.normalize()?;
    let mut toolbar = Toolbar::default();
    for scale in [1., 2., 4.] {
        for placement in Placement::ALL {
            config.placement = placement;
            toolbar.layout(&config, [800, 600], scale, true);
            if placement == Placement::Hidden {
                assert_eq!(toolbar.count, 0);
                assert!(!toolbar.contains([50., 50.]));
                continue;
            }
            assert_eq!(toolbar.count, 32);
            let [x1, y1, x2, y2] = toolbar.bounds;
            assert!(x1 >= 0. && y1 >= 0. && x2 <= 800. && y2 <= 600. - 20. * scale);
            for button in &toolbar.buttons[..toolbar.count] {
                let [a, b, c, d] = button.rect;
                assert!(a >= x1 && b >= y1 && c <= x2 && d <= y2);
                assert!(button.rect.iter().all(|v| v.fract() == 0.));
                assert_eq!(toolbar.hit([(a + c) / 2., (b + d) / 2.]), button.action);
            }
        }
    }
    config.actions.push(Action::Save.id());
    assert!(config.normalize().is_err());
    Ok(())
}

#[test]
fn preferences_migrate_old_shape_and_normalize_unknown_toolbar_ids() -> R {
    let temp = Temp::new()?;
    let path = temp.0.join("preferences.json");
    let profile = Preferences::defaults()?;
    let mut legacy = serde_json::to_value(&profile)?;
    legacy
        .as_object_mut()
        .ok_or("profile object")?
        .remove("toolbar");
    legacy
        .as_object_mut()
        .ok_or("profile object")?
        .remove("status_bar");
    fs::write(&path, serde_json::to_vec(&legacy)?)?;
    let restored = preferences::read(&path)?;
    assert!(restored.status_bar);
    assert_eq!(restored.toolbar.actions, Config::default().actions);
    let mut profile = restored;
    profile.toolbar.actions = vec![
        Action::Save.id(),
        "future-missing-action".into(),
        Action::Save.id(),
        Action::PanView.id(),
    ];
    profile.toolbar.placement = Placement::Floating;
    profile.toolbar.floating = [111, 222];
    fs::write(&path, serde_json::to_vec(&profile)?)?;
    let restored = preferences::read(&path)?;
    assert_eq!(restored.toolbar.actions, vec![Action::Save.id()]);
    assert_eq!(restored.toolbar.placement, Placement::Floating);
    assert_eq!(restored.toolbar.floating, [111, 222]);
    Ok(())
}

#[test]
fn shared_fork_retains_sources_but_offline_editor_cannot_mutate_authority() -> R {
    let mut original = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    original.apply(Command::AddSource(Source::embedded(SourceId::new(7)?)))?;
    let fork = original.fork(DocumentId::new(2)?);
    assert_eq!(original.id().value(), 1);
    assert_eq!(fork.id().value(), 2);
    assert_eq!(fork.sources().count(), 1);
    assert_eq!(
        fork.source(SourceId::new(7)?),
        original.source(SourceId::new(7)?)
    );
    let mut offline = DocumentEditor::shared(fork);
    assert!(
        offline
            .execute(Command::AddSource(Source::embedded(SourceId::new(8)?)))
            .is_err()
    );
    assert!(offline.undo().is_err());
    assert!(offline.redo().is_err());
    assert_eq!(offline.document().sources().count(), 1);
    assert_eq!(offline.pending_backend_requests(), 0);
    assert!(!offline.is_dirty());
    assert_eq!(original.sources().count(), 1);
    Ok(())
}

#[test]
fn shared_companion_binds_identity_and_never_overwrites_existing_metadata() -> R {
    let temp = Temp::new()?;
    let local = temp.0.join("episode.tack");
    fs::write(&local, b"untouched local original")?;
    let snapshot = sharing::proposed(&local, false);
    assert_eq!(snapshot.file_name().ok_or("name")?, "episode-shared.tack");
    let id = DocumentId::new(2)?;
    let board = tack_shared::WireId::new(id.value())?.to_string();
    let descriptor = Descriptor {
        version: 1,
        board: board.clone(),
        owner: Some(sharing::host_identity(&temp.0)?),
        invite: format!("tack://192.168.1.2:7337/{board}"),
    };
    assert!(sharing::owns(&temp.0, &descriptor));
    descriptor.write_new(&snapshot)?;
    let before = fs::read(sharing::sidecar(&snapshot))?;
    let restored = Descriptor::read(&snapshot, id)?.ok_or("descriptor")?;
    assert_eq!(restored.address()?.board.value(), id.value());
    assert!(Descriptor::read(&snapshot, DocumentId::new(3)?).is_err());
    assert!(descriptor.write_new(&snapshot).is_err());
    assert_eq!(fs::read(sharing::sidecar(&snapshot))?, before);
    assert_eq!(fs::read(&local)?, b"untouched local original");
    let mut oversized_identity = descriptor.owner.clone().ok_or("owner")?.into_bytes();
    oversized_identity.extend_from_slice(&[b' '; 4096]);
    fs::write(temp.0.join("hosting-identity"), oversized_identity)?;
    assert!(!sharing::owns(&temp.0, &descriptor));
    fs::write(sharing::sidecar(&snapshot), vec![b' '; 4097])?;
    assert!(Descriptor::read(&snapshot, id).is_err());
    Ok(())
}

// Small uncompressed PNG producer: owned test pixels, no image dependency.
fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(tag);
        out.extend_from_slice(data);
        out.extend_from_slice(&crc32fast::hash(&out[start..]).to_be_bytes());
    }
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    let mut pixels = Vec::new();
    for _ in 0..height {
        pixels.extend_from_slice(&[0]);
        for _ in 0..width {
            pixels.extend_from_slice(&color);
        }
    }
    let len = pixels.len() as u16;
    let mut deflate = vec![0x78, 0x01, 0x01];
    deflate.extend_from_slice(&len.to_le_bytes());
    deflate.extend_from_slice(&(!len).to_le_bytes());
    deflate.extend_from_slice(&pixels);
    let (mut a, mut b) = (1u32, 0u32);
    for byte in pixels {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    deflate.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut out, b"IDAT", &deflate);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[test]
fn startup_icons_are_bounded_repairable_and_edited_pixels_replace_previous_load() -> R {
    let temp = Temp::new()?;
    for name in toolbar_icons::NAMES {
        fs::write(
            temp.0.join(format!("{name}.png")),
            png(16, 16, [20, 30, 40, 255]),
        )?;
    }
    let arrow = temp.0.join("arrow.png");
    let first = toolbar_icons::load(&temp.0);
    assert_eq!(
        (first.width, first.height, first.rgba.len()),
        (128, 64, 32768)
    );
    fs::write(&arrow, png(16, 16, [90, 80, 70, 255]))?;
    let second = toolbar_icons::load(&temp.0);
    assert_ne!(first.rgba, second.rgba);
    let index = toolbar_icons::index(Action::SelectTool(tack_app::actions::Tool::Arrow));
    let offset = (index % 8 * 16 + index / 8 * 16 * 128) * 4;
    assert_eq!(&second.rgba[offset..offset + 4], &[90, 80, 70, 255]);
    for invalid in [
        b"corrupt PNG".to_vec(),
        png(17, 16, [0, 0, 0, 255]),
        png(16, 16, [0, 0, 0, 127]),
        vec![0; 16385],
    ] {
        fs::write(&arrow, invalid)?;
        assert!(toolbar_icons::read(&arrow).is_err());
        let a = toolbar_icons::load(&temp.0);
        let b = toolbar_icons::load(&temp.0);
        assert_eq!(a.rgba.len(), 32768);
        assert_eq!(a.rgba, b.rgba);
    }
    fs::remove_file(&arrow)?;
    assert!(toolbar_icons::read(&arrow).is_err());
    assert_eq!(toolbar_icons::load(&temp.0).rgba.len(), 32768);
    Ok(())
}
