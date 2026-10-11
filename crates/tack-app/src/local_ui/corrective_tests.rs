use super::*;
type R = Result<(), tack_assets::AssetError>;
#[test]
fn keymap_category_headers_never_select_and_every_action_has_a_row() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let mut ui = LocalUi::new(Panel::Keymap);
    let c = Camera::new([800, 600]);
    let mut g = ImageGizmo::default();
    for action in Action::ALL {
        assert!(
            ui.actions(&map)
                .iter()
                .any(|row| shortcut_matches(*row, action)),
            "Missing {action:?}"
        );
    }
    ui.draw(&mut g, &c, &map, &p);
    assert!(ui.key_rows[0].1[1] > ui.layout[3]);
    ui.selected = 3;
    ui.cursor = [35., ui.layout[3] + 5.];
    ui.handle(
        &WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: winit::event::MouseButton::Left,
        },
        &mut map,
        &mut p,
    );
    assert_eq!(ui.selected, 3);
    let (index, r) = *ui.key_rows.last().ok_or("row")?;
    ui.cursor = [35., r[1] + 5.];
    ui.handle(
        &WindowEvent::MouseInput {
            device_id: winit::event::DeviceId::dummy(),
            state: ElementState::Pressed,
            button: winit::event::MouseButton::Left,
        },
        &mut map,
        &mut p,
    );
    assert_eq!(ui.selected, index);
    Ok(())
}
#[test]
fn toolbar_set_editor_changes_content_and_reports_name_without_changing_layout() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let mut ui = LocalUi::new(Panel::Toolbar);
    let before = p.toolbar.clone();
    assert!(matches!(
        ui.toolbar_command(Command::ToolbarSetDuplicate, &mut p),
        Some(UiResult::PreferencesChanged)
    ));
    assert_eq!(p.toolbar.actions, before.actions);
    assert_eq!(p.toolbar.scale, before.scale);
    p.rename_toolbar_set("DRAW")?;
    ui.toolbar_command(Command::ToolbarSetPrevious, &mut p);
    assert_eq!(ui.message, "Toolbar: DEFAULT");
    ui.toolbar_command(Command::ToolbarSetNext, &mut p);
    assert_eq!(ui.message, "Toolbar: DRAW");
    assert!(matches!(
        ui.toolbar_command(Command::ToolbarImportProfile, &mut p),
        Some(UiResult::Action(Action::ImportProfile))
    ));
    assert!(matches!(
        ui.toolbar_command(Command::ToolbarExportProfile, &mut p),
        Some(UiResult::Action(Action::ExportProfile))
    ));
    ui.toolbar_command(Command::ToolbarSetDelete, &mut p);
    assert!(
        ui.toolbar_command(Command::ToolbarSetDelete, &mut p)
            .is_none()
    );
    assert_eq!(p.toolbar_sets.len(), 1);
    ui.draw(
        &mut ImageGizmo::default(),
        &Camera::new([800, 600]),
        &map,
        &p,
    );
    assert!(
        !ui.hits
            .iter()
            .find(|h| h.command == Command::ToolbarSetDelete)
            .ok_or("Delete")?
            .enabled
    );
    ui.toolbar_key(KeyCode::Escape, &mut map, &mut p);
    Ok(())
}
