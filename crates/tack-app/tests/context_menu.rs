use tack_app::{
    actions::{Action, ActionEvent, ActionPhase},
    context_menu::{
        self, Command as MenuCommand, Context, ContextKind, ContextMenu, Group,
        Result as MenuResult,
    },
    image_input::ImageInput,
    image_interaction::SelectionState,
    input::{Modifiers, PhysicalControl, PhysicalEvent, PointerButton},
    selection_commands::{self, Alpha, Order},
};
use tack_core::*;
use winit::{
    event::{ElementState, MouseButton},
    keyboard::KeyCode,
};
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn fixture() -> R<DocumentEditor> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "image.png",
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [200, 100],
    )?))?;
    for i in 1..=5 {
        let t = Transform::new([(i - 1) as f64 * 200., 0.], [100., 80.], 0., [false; 2])?;
        let id = ObjectId::new(i)?;
        let o = match i {
            1 | 2 => DocumentObject::image(id, AssetId::new(1)?, t),
            3 => DocumentObject::annotation(
                id,
                Annotation::new(
                    AnnotationKind::Text(TextObject::new("note".into(), 24., TextAlignment::Left)?),
                    AnnotationStyle::default(),
                ),
                t,
            )?,
            4 => DocumentObject::frame(id, "frame".into(), t)?,
            _ => DocumentObject::annotation(
                id,
                Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
                t,
            )?,
        };
        d.apply(Command::AddObject {
            object: o,
            index: d.object_order().len(),
        })?;
    }
    Ok(DocumentEditor::new(d, 200))
}
fn context(e: &DocumentEditor, ids: &[u128]) -> R<Context> {
    let mut s = SelectionState::default();
    for id in ids {
        s.select(Some(ObjectId::new(*id)?), true);
    }
    Ok(Context::selection(e, &s, false, false))
}
#[test]
fn context_resolution_and_real_available_commands_are_specific() -> R {
    let e = fixture()?;
    let map = tack_app::image_input::product_keymap()?;
    for (ids, kind) in [
        (&[][..], ContextKind::Canvas),
        (&[1][..], ContextKind::Image),
        (&[1, 2][..], ContextKind::Multiple),
        (&[3][..], ContextKind::Note),
        (&[4][..], ContextKind::Frame),
        (&[5][..], ContextKind::Annotation),
    ] {
        let c = context(&e, ids)?;
        assert_eq!(c.kind, kind);
        let rows = context_menu::items(c, None, &map);
        let contains = |a| rows.iter().any(|i| i.command == MenuCommand::Action(a));
        let mut app_context = c;
        app_context.kind = ContextKind::Application;
        let app_rows = context_menu::items(app_context, None, &map);
        assert_eq!(
            rows[..app_rows.len()]
                .iter()
                .map(|i| (i.label, i.command, i.enabled))
                .collect::<Vec<_>>(),
            app_rows
                .iter()
                .map(|i| (i.label, i.command, i.enabled))
                .collect::<Vec<_>>()
        );
        assert_eq!(rows[app_rows.len()].command, MenuCommand::Heading);
        if kind == ContextKind::Canvas {
            assert_eq!(rows.len(), 14);
            assert!(contains(Action::ImportImages));
            assert!(contains(Action::Preferences));
            assert!(contains(Action::KeymapEditor));
            assert!(contains(Action::About));
            assert!(!contains(Action::Save));
        }
        if kind == ContextKind::Image {
            assert!(contains(Action::CropMode));
            assert!(contains(Action::RelinkSource));
        } else {
            assert!(!contains(Action::CropMode));
            assert!(!contains(Action::FlipHorizontal));
        }
        if kind == ContextKind::Note {
            assert!(contains(Action::RenameFrame));
        }
        if kind == ContextKind::Frame {
            assert!(contains(Action::FocusFrame));
        }
        assert!(!c.enabled(Action::Undo));
        assert!(!c.enabled(Action::Redo));
    }
    let image = context(&e, &[1])?;
    assert!(!image.enabled(Action::GroupSelection));
    assert!(!image.enabled(Action::UngroupSelection));
    assert!(context(&e, &[1, 2])?.enabled(Action::GroupSelection));
    assert!(!context(&e, &[1, 3])?.enabled(Action::GroupSelection));
    assert!(context(&e, &[1, 3])?.enabled(Action::Layout(tack_app::spatial_layout::Layout::Left)));
    let app = ContextMenu::application(context(&e, &[4])?, &Camera::new([800, 600]), &map);
    assert!(
        context_menu::items(app.context, Some(Group::View), &map)
            .iter()
            .find(|i| i.command == MenuCommand::Action(Action::FocusFrame))
            .ok_or("expected menu entry or object")?
            .enabled
    );
    Ok(())
}
#[test]
fn right_click_preserves_multi_selection_targets_other_object_and_empty() -> R {
    let e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut cam = Camera::new([800, 600]);
    for i in [1, 2] {
        input.images.selection.select(Some(ObjectId::new(i)?), true);
    }
    input.cursor_moved(cam.world_to_screen([0., 0.]), &e, &mut cam)?;
    input.context_selection(&e, &cam);
    assert_eq!(input.images.selection.len(), 2);
    input.cursor_moved(cam.world_to_screen([400., 0.]), &e, &mut cam)?;
    input.context_selection(&e, &cam);
    assert_eq!(context(&e, &[3])?.kind, ContextKind::Note);
    assert!(input.images.selection.contains(ObjectId::new(3)?));
    assert_eq!(input.images.selection.len(), 1);
    input.cursor_moved(cam.world_to_screen([1200., 300.]), &e, &mut cam)?;
    input.context_selection(&e, &cam);
    assert!(input.images.selection.is_empty());
    assert_eq!(e.undo_len(), 0);
    Ok(())
}
#[test]
fn menu_edges_submenus_scroll_dismiss_and_never_delete_on_right_arrow() -> R {
    let e = fixture()?;
    let map = tack_app::image_input::product_keymap()?;
    for screen in [[800, 600], [1024, 768]] {
        for scale in [1., 2., 3., 8.] {
            let mut camera = Camera::new(screen);
            camera.set_ui_scale(scale);
            for ids in [&[1][..], &[1, 2][..], &[3][..], &[4][..], &[5][..], &[][..]] {
                for anchor in [[0., 0.], [screen[0] as f64 - 1., screen[1] as f64 - 1.]] {
                    let c = context(&e, ids)?;
                    let root = context_menu::items(c, None, &map);
                    for index in 0..root.len() {
                        let mut menu = ContextMenu::new(c, anchor, &camera, &map);
                        for _ in 0..root[..=index]
                            .iter()
                            .filter(|i| i.command != MenuCommand::Heading)
                            .count()
                        {
                            menu.key(KeyCode::ArrowDown, &map, &camera);
                        }
                        let outcome = menu.key(KeyCode::ArrowRight, &map, &camera);
                        assert_eq!(outcome, MenuResult::None, "Right must never invoke a leaf");
                        for rect in menu.rectangles() {
                            assert!(rect.x >= 0 && rect.y >= 0);
                            assert!((rect.x + rect.width) as f64 * scale <= screen[0] as f64);
                            assert!((rect.y + rect.height) as f64 * scale <= screen[1] as f64);
                        }
                        if menu.rectangles().len() == 2 {
                            assert_eq!(menu.key(KeyCode::Escape, &map, &camera), MenuResult::None);
                            assert_eq!(menu.rectangles().len(), 1, "Escape returns to parent");
                        }
                        assert_eq!(
                            menu.key(KeyCode::Escape, &map, &camera),
                            MenuResult::Dismiss
                        );
                        menu.move_pointer([-100., -100.], &map, &camera);
                        assert_eq!(menu.click(&map, &camera), MenuResult::Dismiss);
                    }
                }
            }
        }
    }
    let camera = Camera::new([800, 600]);
    let mut menu = ContextMenu::new(context(&e, &[1])?, [10., 10.], &camera, &map);
    let delete = menu
        .root_items()
        .iter()
        .position(|i| i.command == MenuCommand::Action(Action::DeleteSelection))
        .ok_or("expected menu entry or object")?;
    for _ in 0..menu.root_items()[..=delete]
        .iter()
        .filter(|i| i.command != MenuCommand::Heading)
        .count()
    {
        menu.key(KeyCode::ArrowDown, &map, &camera);
    }
    assert_eq!(
        menu.root_items()[delete].command,
        MenuCommand::Action(Action::DeleteSelection)
    );
    assert_eq!(
        menu.key(KeyCode::Enter, &map, &camera),
        MenuResult::Action(Action::DeleteSelection)
    );
    Ok(())
}
fn key(
    input: &mut ImageInput,
    e: &mut DocumentEditor,
    cam: &mut Camera,
    mods: Modifiers,
    k: KeyCode,
) -> R {
    input.physical(PhysicalEvent::Modifiers(mods), e, cam)?;
    for state in [ElementState::Pressed, ElementState::Released] {
        input.physical(
            PhysicalEvent::Button {
                control: PhysicalControl::LogicalKey(
                    tack_app::input::LogicalKey::from_legacy(k).ok_or("logical key")?,
                ),
                state,
                repeat: false,
            },
            e,
            cam,
        )?;
    }
    Ok(())
}
#[test]
fn menu_and_keyboard_share_command_history_and_shortcut_display() -> R {
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut cam = Camera::new([800, 600]);
    input
        .images
        .selection
        .select(Some(ObjectId::new(1)?), false);
    let original = e.document().clone();
    input.dispatch(
        ActionEvent {
            action: Action::FlipHorizontal,
            phase: ActionPhase::Invoke,
        },
        &mut e,
        &mut cam,
    )?;
    let flipped = e.document().clone();
    assert_ne!(original, flipped);
    assert_eq!(e.undo_len(), 1);
    key(
        &mut input,
        &mut e,
        &mut cam,
        Modifiers::CONTROL,
        KeyCode::KeyZ,
    )?;
    assert_eq!(e.document(), &original);
    key(
        &mut input,
        &mut e,
        &mut cam,
        Modifiers::CONTROL.union(Modifiers::SHIFT),
        KeyCode::KeyZ,
    )?;
    assert_eq!(e.document(), &flipped);
    input.dispatch(
        ActionEvent {
            action: Action::Opacity(Alpha::Half),
            phase: ActionPhase::Invoke,
        },
        &mut e,
        &mut cam,
    )?;
    assert_eq!(
        e.document()
            .object_render_data(ObjectId::new(1)?)
            .ok_or("expected menu entry or object")?
            .opacity
            .value(),
        0.5
    );
    assert_eq!(e.undo_len(), 2);
    let m = &input.keymap;
    assert_eq!(context_menu::shortcut(m, Action::Undo), "Ctrl+Z");
    assert_eq!(context_menu::shortcut(m, Action::Redo), "Ctrl+Shift+Z");
    assert!(!context_menu::shortcut(m, Action::Order(Order::Front)).is_empty());
    input.keymap.unassign(Action::Undo);
    assert!(context_menu::shortcut(&input.keymap, Action::Undo).is_empty());
    Ok(())
}
#[test]
fn native_drag_route_commits_once_after_many_motion_events() -> R {
    let mut e = fixture()?;
    let original = e.document().clone();
    let mut input = ImageInput::new()?;
    let mut cam = Camera::new([800, 600]);
    input.cursor_moved(cam.world_to_screen([0., 0.]), &e, &mut cam)?;
    let edge = |state| PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
        state,
        repeat: false,
    };
    input.physical(edge(ElementState::Pressed), &mut e, &mut cam)?;
    for i in 1..=200 {
        input.cursor_moved([400. + i as f64 / 2., 300. + i as f64 / 4.], &e, &mut cam)?;
    }
    assert_eq!(e.document(), &original);
    assert_eq!(e.undo_len(), 0);
    input.physical(edge(ElementState::Released), &mut e, &mut cam)?;
    assert_eq!(e.undo_len(), 1);
    assert_ne!(e.document(), &original);
    key(
        &mut input,
        &mut e,
        &mut cam,
        Modifiers::CONTROL,
        KeyCode::KeyZ,
    )?;
    assert_eq!(e.document(), &original);
    Ok(())
}
#[test]
fn selected_order_preserves_relative_order_with_exact_single_undo_for_all_subsets() -> R {
    for mask in 0u8..32 {
        for direction in [Order::Forward, Order::Front, Order::Backward, Order::Back] {
            let mut e = fixture()?;
            let original = e.document().clone();
            let ids: Vec<_> = e
                .document()
                .object_order()
                .iter()
                .copied()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, id)| id)
                .collect();
            let selected = |id: &ObjectId| ids.contains(id);
            let mut wanted = original.object_order().to_vec();
            match direction {
                Order::Front => wanted.sort_by_key(selected),
                Order::Back => wanted.sort_by_key(|id| !selected(id)),
                Order::Forward => {
                    for i in (0..4).rev() {
                        if selected(&wanted[i]) && !selected(&wanted[i + 1]) {
                            wanted.swap(i, i + 1);
                        }
                    }
                }
                Order::Backward => {
                    for i in 1..5 {
                        if selected(&wanted[i]) && !selected(&wanted[i - 1]) {
                            wanted.swap(i, i - 1);
                        }
                    }
                }
            }
            let changed = e.execute(selection_commands::order(
                e.document(),
                ids.into_iter(),
                direction,
            ))?;
            assert_eq!(e.document().object_order(), wanted);
            assert_eq!(e.undo_len(), usize::from(changed));
            if changed {
                e.undo()?;
                assert_eq!(e.document(), &original);
                e.redo()?;
                assert_eq!(e.document().object_order(), wanted);
            }
        }
    }
    Ok(())
}

#[test]
fn popup_cancels_preview_preserves_modifiers_and_order_alpha_roundtrip() -> R {
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut cam = Camera::new([800, 600]);
    input
        .images
        .selection
        .select(Some(ObjectId::new(1)?), false);
    let before = e.document().clone();
    input
        .images
        .begin(tack_app::image_interaction::GestureKind::Move, [0., 0.], &e)?;
    input.images.update([30., 20.])?;
    assert!(input.images.active());
    input.physical(
        PhysicalEvent::Modifiers(Modifiers::CONTROL),
        &mut e,
        &mut cam,
    )?;
    input.suspend_for_menu(&mut e, &mut cam)?;
    assert_eq!(input.modifiers(), Modifiers::CONTROL);
    assert!(!input.images.active());
    assert_eq!(e.document(), &before);
    assert_eq!(e.undo_len(), 0);
    let mut menu = ContextMenu::new(context(&e, &[1])?, [200., 200.], &cam, &input.keymap);
    menu.set_modifiers(input.modifiers());
    assert_eq!(
        menu.key(KeyCode::KeyZ, &input.keymap, &cam),
        MenuResult::None
    );
    for action in [Action::Opacity(Alpha::Half), Action::Order(Order::Front)] {
        input.dispatch(
            ActionEvent {
                action,
                phase: ActionPhase::Invoke,
            },
            &mut e,
            &mut cam,
        )?;
    }
    assert_eq!(e.undo_len(), 2);
    let after = e.document().clone();
    assert_eq!(
        menu.key(KeyCode::KeyZ, &input.keymap, &cam),
        MenuResult::None
    ); // stale availability is rechecked by the native owner
    input.physical(
        PhysicalEvent::Button {
            control: PhysicalControl::LogicalKey(
                tack_app::input::LogicalKey::from_legacy(KeyCode::KeyZ).ok_or("logical key")?,
            ),
            state: ElementState::Pressed,
            repeat: false,
        },
        &mut e,
        &mut cam,
    )?;
    assert_eq!(e.undo_len(), 1);
    input.physical(
        PhysicalEvent::Button {
            control: PhysicalControl::LogicalKey(
                tack_app::input::LogicalKey::from_legacy(KeyCode::KeyZ).ok_or("logical key")?,
            ),
            state: ElementState::Released,
            repeat: false,
        },
        &mut e,
        &mut cam,
    )?;
    input.physical(
        PhysicalEvent::Modifiers(Modifiers::CONTROL.union(Modifiers::SHIFT)),
        &mut e,
        &mut cam,
    )?;
    input.physical(
        PhysicalEvent::Button {
            control: PhysicalControl::LogicalKey(
                tack_app::input::LogicalKey::from_legacy(KeyCode::KeyZ).ok_or("logical key")?,
            ),
            state: ElementState::Pressed,
            repeat: false,
        },
        &mut e,
        &mut cam,
    )?;
    assert_eq!(e.document(), &after);
    let path = std::env::temp_dir().join(format!(
        "tack-1h-menu-{}.tack",
        tack_storage::new_document_id()?.value()
    ));
    tack_storage::save(&path, e.document(), vec![])?;
    let loaded = tack_storage::TackFile::open(&path)?;
    assert_eq!(loaded.document, after);
    std::fs::remove_file(path)?;
    Ok(())
}
#[test]
fn compact_modal_rectangles_fit_small_displays_without_minimum_size_overflow() -> R {
    let map = tack_app::image_input::product_keymap()?;
    let profile = tack_app::preferences::Preferences::defaults()?;
    for screen in [[800, 600], [1024, 768]] {
        for scale in [1., 2., 4., 8.] {
            let mut cam = Camera::new(screen);
            cam.set_ui_scale(scale);
            for panel in [
                tack_app::local_ui::Panel::Preferences,
                tack_app::local_ui::Panel::Keymap,
                tack_app::local_ui::Panel::Close,
                tack_app::local_ui::Panel::Recovery,
                tack_app::local_ui::Panel::Error,
            ] {
                let mut ui = tack_app::local_ui::LocalUi::new(panel);
                let mut gizmo = tack_app::image_gizmo::ImageGizmo::default();
                ui.draw(&mut gizmo, &cam, &map, &profile);
                for quad in gizmo.quads.iter().filter(|q| q.bitmap.is_none()) {
                    for p in quad.points {
                        let [x, y] = cam.world_to_screen(p);
                        assert!(
                            x >= 0. && y >= 0. && x <= screen[0] as f64 && y <= screen[1] as f64,
                            "{panel:?} {screen:?} {scale} {x} {y}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn compact_submenus_leave_parent_preferences_reachable_by_pointer() -> R {
    let editor = fixture()?;
    let map = tack_app::image_input::product_keymap()?;
    for scale in [1., 2.] {
        let mut camera = Camera::new([800, 600]);
        camera.set_ui_scale(scale);
        for anchor in [[0., 0.], [200., 200.], [799., 599.]] {
            let mut menu = ContextMenu::new(context(&editor, &[1])?, anchor, &camera, &map);
            menu.key(KeyCode::ArrowDown, &map, &camera);
            menu.key(KeyCode::ArrowRight, &map, &camera);
            let rectangles = menu.rectangles();
            assert_eq!(rectangles.len(), 2);
            let (root, child) = (rectangles[0], rectangles[1]);
            assert!(
                root.x + root.width <= child.x || child.x + child.width <= root.x,
                "overlapping columns {scale} {anchor:?}"
            );
            let index = menu
                .root_items()
                .iter()
                .position(|i| i.command == MenuCommand::Action(Action::Preferences))
                .ok_or("preferences")?;
            menu.move_pointer(
                [
                    (root.x + 18) as f64 * scale,
                    (root.y + 2 + index as i32 * 18 + 2) as f64 * scale,
                ],
                &map,
                &camera,
            );
            let mut gizmo = tack_app::image_gizmo::ImageGizmo::default();
            menu.draw(&mut gizmo, &camera);
            assert_eq!(
                menu.click(&map, &camera),
                MenuResult::Action(Action::Preferences)
            );
        }
    }
    Ok(())
}

#[test]
fn about_uses_same_semantic_action_in_f10_and_context_menu() -> R {
    let editor = fixture()?;
    let map = tack_app::image_input::product_keymap()?;
    let camera = Camera::new([800, 600]);
    let context = context(&editor, &[])?;
    for mut menu in [
        ContextMenu::application(context, &camera, &map),
        ContextMenu::new(context, [10., 10.], &camera, &map),
    ] {
        let index = menu
            .root_items()
            .iter()
            .position(|row| row.command == MenuCommand::Action(Action::About))
            .ok_or("About absent")?;
        for _ in 0..menu.root_items()[..=index]
            .iter()
            .filter(|row| row.command != MenuCommand::Heading)
            .count()
        {
            menu.key(KeyCode::ArrowDown, &map, &camera);
        }
        assert_eq!(
            menu.key(KeyCode::Enter, &map, &camera),
            MenuResult::Action(Action::About)
        );
    }
    Ok(())
}

#[test]
fn original_export_is_enabled_for_linked_and_embedded_single_images() -> R {
    let mut editor = fixture()?;
    let map = tack_app::image_input::product_keymap()?;
    for embedded in [false, true] {
        if embedded {
            editor.execute(Command::SetSource(Source::from_descriptor(
                SourceId::new(1)?,
                SourceLocation::Embedded,
                2,
                None,
            )?))?;
        }
        let image = context(&editor, &[1])?;
        assert!(image.enabled(Action::SaveOriginalAs));
        let rows = context_menu::items(image, Some(Group::Source), &map);
        assert!(
            rows.iter().any(
                |row| row.command == MenuCommand::Action(Action::SaveOriginalAs) && row.enabled
            )
        );
        assert_eq!(image.enabled(Action::OpenSource), !embedded);
        assert_eq!(image.enabled(Action::RevealSource), !embedded);
        assert_eq!(image.enabled(Action::CopySourcePath), !embedded);
    }
    for ids in [&[][..], &[1, 2][..], &[3][..]] {
        assert!(!context(&editor, ids)?.enabled(Action::SaveOriginalAs));
    }
    Ok(())
}
