use std::{fs, path::PathBuf};
use tack_app::{
    actions::{Action, ActionEvent, ActionPhase, Tool},
    image_input::ImageInput,
    input::{Modifiers, PhysicalControl, PhysicalEvent},
    portable_profile,
    preferences::{self, Preferences, RecentPath},
    toolbar::{Placement, SEPARATOR},
};
use tack_core::{Camera, Document, DocumentEditor, DocumentId, DocumentLimits};
use winit::{
    event::ElementState,
    keyboard::{KeyCode, PhysicalKey},
};
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
struct Root(PathBuf);
impl Root {
    fn new() -> R<Self> {
        let p = std::env::temp_dir().join(format!(
            "tack-profile-test-{:032x}",
            tack_storage::new_document_id()?.value()
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
#[test]
fn full_environment_roundtrip_and_keymap_only_exports_stay_separate() -> R {
    let r = Root::new()?;
    let mut p = Preferences::defaults()?;
    p.grid = true;
    p.snapping = true;
    p.embedded_import = false;
    p.sampling = "Nearest".into();
    p.handle_size = 13;
    p.hit_radius = 12;
    p.toolbar.placement = Placement::Right;
    p.toolbar.scale = 2;
    p.status_bar = false;
    p.rename_toolbar_set("IMAGE")?;
    p.new_toolbar_set(true)?;
    p.rename_toolbar_set("DRAW")?;
    p.toolbar.actions = vec![
        Action::SelectTool(Tool::Text).id(),
        SEPARATOR.into(),
        Action::Undo.id(),
    ];
    p.sync_active_set();
    p.toolbar.floating = [800, 650];
    p.toolbar.offset = 650;
    p.toolbar.offset_set = true;
    p.recent
        .push(RecentPath::native(&r.0.join("private-board.tack"))?);
    p.last_board_directory = Some(RecentPath::native(&r.0)?);
    p.keyset.path = Some(RecentPath::native(&r.0.join("private.tackey"))?);
    p.keymap
        .iter_mut()
        .find(|b| b.action == Action::Save.id())
        .ok_or("Save")?
        .control = PhysicalControl::LogicalKey(tack_app::input::LogicalKey::Character('q'));
    // Existing native binding strings are validated, including Hold/Release.
    let expected = portable_profile::portable(&p)?;
    portable_profile::export(&r.0.join("studio"), &p)?;
    let file = r.0.join("studio.tackprofile");
    let actual = portable_profile::read(&file)?;
    assert_eq!(actual, expected);
    assert_eq!(actual.toolbar_set_name(), "DRAW");
    assert_eq!(actual.toolbar_sets.len(), 2);
    assert!(actual.snapping && actual.grid);
    assert!(!fs::read_to_string(&file)?.contains("private-board"));
    assert!(!fs::read_to_string(&file)?.contains(r.0.to_string_lossy().as_ref()));
    preferences::export_keymap(&r.0.join("keys"), &p)?;
    let keys = preferences::read_keymap(&r.0.join("keys.tackey"))?;
    assert_eq!(keys.keymap, p.keymap);
    assert_eq!(keys.toolbar_sets.len(), 1);
    assert!(!keys.grid);
    assert!(portable_profile::read(&r.0.join("keys.tackey")).is_err());
    assert!(portable_profile::export(&r.0.join("studio"), &p).is_err());
    Ok(())
}
#[test]
fn set_content_cycles_without_layout_or_tool_mutation_and_never_deletes_last() -> R {
    let mut p = Preferences::defaults()?;
    let layout = p.toolbar.clone();
    p.rename_toolbar_set("IMAGE")?;
    p.new_toolbar_set(false)?;
    p.rename_toolbar_set("DRAW")?;
    p.toolbar.actions = vec![Action::Undo.id()];
    p.sync_active_set();
    p.new_toolbar_set(true)?;
    p.rename_toolbar_set("COPY")?;
    assert_eq!(p.toolbar.actions, vec![Action::Undo.id()]);
    assert!(p.rename_toolbar_set(" image ").is_err());
    p.delete_toolbar_set()?;
    assert_eq!(p.toolbar_set_name(), "DRAW");
    p.cycle_toolbar_set(false)?;
    assert_eq!(p.toolbar.actions, layout.actions);
    assert_eq!(p.toolbar.placement, layout.placement);
    assert_eq!(p.toolbar.scale, layout.scale);
    let mut input = ImageInput::new()?;
    let mut editor = DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        16,
    );
    let mut camera = Camera::new([800, 600]);
    input.dispatch(
        ActionEvent {
            action: Action::SelectTool(Tool::Scribble),
            phase: ActionPhase::Invoke,
        },
        &mut editor,
        &mut camera,
    )?;
    let active = input.active_tool();
    input.physical(
        PhysicalEvent::Modifiers(Modifiers::CONTROL),
        &mut editor,
        &mut camera,
    )?;
    for (key, action) in [
        (KeyCode::PageDown, Action::NextToolbarSet),
        (KeyCode::PageUp, Action::PreviousToolbarSet),
    ] {
        input.physical(
            PhysicalEvent::Keyboard {
                physical: PhysicalKey::Code(key),
                logical: Some(tack_app::input::LogicalKey::Named(
                    if key == KeyCode::PageDown {
                        winit::keyboard::NamedKey::PageDown
                    } else {
                        winit::keyboard::NamedKey::PageUp
                    },
                )),
                state: ElementState::Pressed,
                repeat: false,
            },
            &mut editor,
            &mut camera,
        )?;
        assert_eq!(input.pending_local.take(), Some(action));
        assert_eq!(input.active_tool(), active);
        input.physical(
            PhysicalEvent::Keyboard {
                physical: PhysicalKey::Code(key),
                logical: Some(tack_app::input::LogicalKey::Named(
                    if key == KeyCode::PageDown {
                        winit::keyboard::NamedKey::PageDown
                    } else {
                        winit::keyboard::NamedKey::PageUp
                    },
                )),
                state: ElementState::Released,
                repeat: false,
            },
            &mut editor,
            &mut camera,
        )?;
    }
    p.delete_toolbar_set()?;
    assert!(p.delete_toolbar_set().is_err());
    assert_eq!(p.toolbar_sets.len(), 1);
    Ok(())
}
#[test]
fn legacy_fields_migrate_and_malformed_profiles_fail_without_changing_settings() -> R {
    let r = Root::new()?;
    let p = Preferences::defaults()?;
    portable_profile::export(&r.0.join("p.tackprofile"), &p)?;
    let path = r.0.join("p.tackprofile");
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
    let prefs = v["preferences"].as_object_mut().ok_or("prefs")?;
    prefs.remove("toolbar_sets");
    prefs.remove("active_toolbar_set");
    prefs.remove("snapping");
    fs::write(&path, serde_json::to_vec(&v)?)?;
    assert_eq!(portable_profile::read(&path)?.toolbar_set_name(), "DEFAULT");
    v["version"] = 900.into();
    fs::write(&path, serde_json::to_vec(&v)?)?;
    assert!(portable_profile::read(&path).is_err());
    v["version"] = 1.into();
    v["preferences"]
        .as_object_mut()
        .ok_or("prefs")?
        .remove("keymap");
    fs::write(&path, serde_json::to_vec(&v)?)?;
    assert!(portable_profile::read(&path).is_err());
    assert_eq!(p, Preferences::defaults()?);
    Ok(())
}
#[test]
fn concurrent_set_edits_conflict_atomically_but_independent_settings_merge() -> R {
    let base = Preferences::defaults()?;
    let mut a = base.clone();
    let mut b = base.clone();
    a.new_toolbar_set(false)?;
    b.rename_toolbar_set("OTHER")?;
    assert!(preferences::merge_settings(&base, &a, &b).is_err());
    b = base.clone();
    b.snapping = true;
    let merged = preferences::merge_settings(&base, &a, &b)?;
    assert!(merged.snapping);
    assert_eq!(merged.toolbar_sets, a.toolbar_sets);
    Ok(())
}
#[test]
fn older_default_profile_gets_new_shortcuts_but_named_custom_keymaps_are_preserved() -> R {
    let r = Root::new()?;
    let mut p = Preferences::defaults()?;
    p.toolbar_sets.clear();
    p.keymap.retain(|b| {
        !matches!(
            b.action.as_str(),
            "NextToolbarSet"
                | "PreviousToolbarSet"
                | "ContextCoarseIncrease"
                | "ContextCoarseDecrease"
        )
    });
    let old = p.keymap.clone();
    preferences::write(&r.0.join("defaults.json"), &p)?;
    let restored = preferences::read(&r.0.join("defaults.json"))?;
    assert!(
        restored
            .keymap()?
            .for_action(Action::NextToolbarSet)
            .next()
            .is_some()
    );
    assert!(old.iter().all(|b| restored.keymap.contains(b)));
    p.keyset.name = "CUSTOM".into();
    preferences::write(&r.0.join("custom.json"), &p)?;
    assert_eq!(preferences::read(&r.0.join("custom.json"))?.keymap, old);
    Ok(())
}
