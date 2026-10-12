use tack_app::{
    actions::{Action, ActionEvent, ActionPhase},
    annotation_tool::StyleAction,
    context_menu::{self, Command as MenuCommand, Context},
    image_input::ImageInput,
    toolbar,
    toolbar_icons::{self, AlignmentState},
};
use tack_core::*;
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const EXCLUDED: &[&str] = &[
    "SelectAll",
    "DeleteSelection",
    "Opacity(Full)",
    "Opacity(ThreeQuarters)",
    "Opacity(Half)",
    "Opacity(Quarter)",
    "Filtering(Default)",
    "Filtering(Smooth)",
    "Filtering(Nearest)",
    "ContextCoarseIncrease",
    "ContextCoarseDecrease",
    "ContextIncrease",
    "ContextDecrease",
    "Layout(SnapToGrid)",
    "FinishScribble",
    "RenameFrame",
    "AnnotationStyle(Fill)",
    "AnnotationStyle(Wider)",
    "AnnotationStyle(Narrower)",
    "AnnotationStyle(LargerText)",
    "AnnotationStyle(SmallerText)",
    "AnnotationStyle(OpacityUp)",
    "AnnotationStyle(OpacityDown)",
    "OpenSource",
    "RevealSource",
    "CopySourcePath",
    "About",
    "CheckForUpdates",
    "NewBoard",
    "CloseBoard",
    "ImportImages",
    "SaveAs",
    "SaveToLocal",
    "Paste",
    "RelinkSource",
    "SaveOriginalAs",
    "ToggleAlwaysOnTop",
    "ToggleNativeTitleBar",
    "MinimizeWindow",
    "MaximizeWindow",
    "ExportSelectedSources",
    "ExportAllSources",
    "Preferences",
    "KeymapEditor",
    "RecentBoards",
    "ImportKeymap",
    "ExportKeymap",
    "SaveKeymap",
    "ExportPreferences",
    "ExportProfile",
    "ImportProfile",
    "AddCameraBookmark",
    "SourceInfo",
    "ToggleToolbar",
    "EditToolbar",
    "ToggleStatusBar",
];
fn fixture() -> R<(DocumentEditor, ImageInput, Camera)> {
    let mut e = DocumentEditor::new(
        Document::new(DocumentId::new(901)?, DocumentLimits::default()),
        128,
    );
    let mut input = ImageInput::new()?;
    let c = Camera::new([800, 600]);
    input.paste_text_note("Note".into(), &mut e, &c)?;
    e.document().objects().next().ok_or("note")?;
    Ok((e, input, c))
}
fn invoke(i: &mut ImageInput, e: &mut DocumentEditor, c: &mut Camera, a: Action) -> R {
    i.dispatch(
        ActionEvent {
            action: a,
            phase: ActionPhase::Invoke,
        },
        e,
        c,
    )?;
    Ok(())
}
#[test]
fn full_exclusion_catalog_retains_actions_and_menu_routes() -> R {
    let (mut e, mut i, _) = fixture()?;
    let note = i.images.selection.ids().next().ok_or("note")?;
    e.execute(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    e.execute(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [16, 16],
    )?))?;
    let t = Transform::new([0.; 2], [100.; 2], 0., [false; 2])?;
    e.execute(Command::AddObject {
        object: DocumentObject::image(ObjectId::new(1)?, AssetId::new(1)?, t),
        index: 1,
    })?;
    e.execute(Command::AddObject {
        object: DocumentObject::frame(ObjectId::new(2)?, "Frame".into(), t)?,
        index: 2,
    })?;
    e.execute(Command::AddObject {
        object: DocumentObject::annotation(
            ObjectId::new(3)?,
            Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
            t,
        )?,
        index: 3,
    })?;
    let map = tack_app::image_input::product_keymap()?;
    let mut routes = std::collections::BTreeSet::new();
    for selected in [
        None,
        Some(note),
        Some(ObjectId::new(1)?),
        Some(ObjectId::new(2)?),
        Some(ObjectId::new(3)?),
    ] {
        i.images.selection.select(selected, false);
        let ctx = Context::selection(&e, &i.images.selection, false, false);
        for item in context_menu::items(ctx, None, &map) {
            match item.command {
                MenuCommand::Action(a) => {
                    routes.insert(a.id());
                }
                MenuCommand::Submenu(g) => {
                    for child in context_menu::items(ctx, Some(g), &map) {
                        if let MenuCommand::Action(a) = child.command {
                            routes.insert(a.id());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let panels = [
        "ImportKeymap",
        "ExportKeymap",
        "SaveKeymap",
        "ExportPreferences",
        "ExportProfile",
        "ImportProfile",
    ];
    for id in EXCLUDED {
        let a = Action::from_id(id).ok_or("missing action")?;
        assert!(!toolbar::eligible(a), "{id}");
        assert!(Action::ALL.contains(&a));
        assert!(
            routes.contains(*id) || panels.contains(id),
            "no menu/panel route: {id}"
        );
    }
    assert!(toolbar::DEFAULT_ACTIONS.into_iter().all(toolbar::eligible));
    assert!(toolbar::eligible(Action::AnnotationStyle(
        StyleAction::AlignText
    )));
    assert!(!toolbar::DEFAULT_ACTIONS.contains(&Action::AnnotationStyle(StyleAction::AlignText)));
    assert!(Action::from_id("SelectTool(RotateView)").is_none());
    assert!(Action::from_id("TemporaryTool(RotateView)").is_none());
    assert!(!toolbar_icons::NAMES.contains(&"rotate_view_tool"));
    Ok(())
}
#[test]
fn alignment_tracks_selection_cycle_undo_mixed_and_real_hard_pixel_assets() -> R {
    let (mut e, mut i, mut c) = fixture()?;
    let initial = e.document().clone();
    for state in [
        AlignmentState::Center,
        AlignmentState::Right,
        AlignmentState::Left,
    ] {
        invoke(
            &mut i,
            &mut e,
            &mut c,
            Action::AnnotationStyle(StyleAction::AlignText),
        )?;
        assert_eq!(
            toolbar_icons::note_alignment(e.document(), i.images.selection.ids(), None),
            state
        );
        assert_eq!(i.status, state.label());
    }
    e.undo()?;
    assert_eq!(
        toolbar_icons::note_alignment(e.document(), i.images.selection.ids(), None),
        AlignmentState::Right
    );
    e.redo()?;
    assert_eq!(e.document(), &initial);
    let first = i.images.selection.ids().next().ok_or("note")?;
    i.paste_text_note("Other".into(), &mut e, &c)?;
    invoke(
        &mut i,
        &mut e,
        &mut c,
        Action::AnnotationStyle(StyleAction::AlignText),
    )?;
    i.images.selection.select(Some(first), true);
    assert_eq!(
        toolbar_icons::note_alignment(e.document(), i.images.selection.ids(), None),
        AlignmentState::Mixed
    );
    i.images.selection.clear();
    assert_eq!(
        toolbar_icons::note_alignment(e.document(), i.images.selection.ids(), None),
        AlignmentState::None
    );
    let mut pixels = Vec::new();
    for state in [
        AlignmentState::Left,
        AlignmentState::Center,
        AlignmentState::Right,
        AlignmentState::Mixed,
    ] {
        let p = toolbar_icons::read(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../gfx/icons")
                .join(format!("{}.png", toolbar_icons::NAMES[state.icon()])),
        )?;
        assert_eq!(p.len(), 1024);
        assert!(p.chunks_exact(4).all(|v| v[3] == 0 || v[3] == 255));
        assert!(!pixels.contains(&p));
        pixels.push(p);
    }
    assert_eq!(toolbar_icons::ATLAS_HEIGHT, 80);
    Ok(())
}
#[test]
fn single_note_adjustment_routes_and_single_grid_snap_are_effective() -> R {
    let (mut e, mut i, mut c) = fixture()?;
    let map = tack_app::image_input::product_keymap()?;
    let ctx = Context::selection(&e, &i.images.selection, false, false);
    let root = context_menu::items(ctx, None, &map);
    assert!(root.iter().any(|x| x.command
        == MenuCommand::Action(Action::Layout(tack_app::spatial_layout::Layout::SnapToGrid))
        && x.enabled));
    assert!(
        root.iter()
            .any(|x| x.command == MenuCommand::Submenu(context_menu::Group::Adjustments))
    );
    let before = e.document().clone();
    let history = e.undo_len();
    invoke(&mut i, &mut e, &mut c, Action::ContextCoarseIncrease)?;
    assert_eq!(e.undo_len(), history + 1);
    assert_ne!(e.document(), &before);
    e.undo()?;
    assert_eq!(e.document(), &before);
    Ok(())
}
