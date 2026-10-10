use super::*;
type R = Result<(), tack_assets::AssetError>;
#[test]
fn toolbar_add_scroll_remove_reorder_and_button_keyboard_are_one_focus() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let mut ui = LocalUi::new(Panel::Toolbar);
    let mut g = ImageGizmo::default();
    let mut c = Camera::new([800, 600]);
    c.set_ui_scale(2.);
    ui.draw(&mut g, &c, &map, &p);
    // First catalog row is the layout item, outside the Action catalog.
    ui.selected = 0;
    ui.toolbar_key(KeyCode::Enter, &mut map, &mut p);
    assert!(ui.toolbar_order);
    assert_eq!(
        p.toolbar.actions.last().map(String::as_str),
        Some(crate::toolbar::SEPARATOR)
    );
    assert_eq!(ui.selected, 12);
    assert!(ui.feedback.active());
    g.quads.clear();
    ui.draw(&mut g, &c, &map, &p);
    assert!(ui.first <= ui.selected && ui.selected < ui.first + ui.visible);
    assert_eq!(ui.first, ui.selected + 1 - ui.visible);
    ui.modifiers = Modifiers::CONTROL;
    ui.toolbar_key(KeyCode::ArrowUp, &mut map, &mut p);
    assert_eq!(ui.selected, 11);
    assert_eq!(p.toolbar.actions[11], crate::toolbar::SEPARATOR);
    ui.modifiers = Modifiers::NONE;
    ui.toolbar_key(KeyCode::Tab, &mut map, &mut p);
    assert_eq!(ui.focus, Some(Command::ToolbarToggle));
    ui.toolbar_key(KeyCode::ArrowRight, &mut map, &mut p);
    assert_eq!(ui.focus, Some(Command::ToolbarRemove));
    ui.toolbar_key(KeyCode::Enter, &mut map, &mut p);
    assert_eq!(p.toolbar.actions.len(), 12);
    assert_eq!(ui.selected, 11);
    assert!(ui.focus.is_none());
    assert!(matches!(
        ui.toolbar_key(KeyCode::Escape, &mut map, &mut p),
        Some(UiResult::Dismiss)
    ));
    Ok(())
}
#[test]
fn toolbar_editor_reuses_icons_and_controls_fit_small_sizes() -> R {
    let p = Preferences::defaults()?;
    let map = p.keymap()?;
    for (size, scale) in [([800, 600], 1.), ([800, 600], 2.), ([1024, 768], 1.)] {
        let mut c = Camera::new(size);
        c.set_ui_scale(scale);
        let mut ui = LocalUi::new(Panel::Toolbar);
        let mut g = ImageGizmo::default();
        ui.draw(&mut g, &c, &map, &p);
        assert!(ui.icons().len() <= 24);
        for icon in ui.icons() {
            assert!(icon.index < 15);
            assert_eq!(icon.rect[2], 16. * scale);
        }
        for h in &ui.hits {
            assert!(
                h.rect[0] >= 0.
                    && h.rect[2] * scale <= f64::from(size[0])
                    && h.rect[3] * scale <= f64::from(size[1])
            );
        }
        assert!(ui.hits.iter().any(|h| h.command == Command::Close));
    }
    Ok(())
}
#[test]
fn pointer_hover_cannot_retarget_a_shortcut_capture() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let mut ui = LocalUi::new(Panel::Keymap);
    ui.draw(
        &mut ImageGizmo::default(),
        &Camera::new([800, 600]),
        &map,
        &p,
    );
    ui.selected = 1;
    ui.capture = true;
    let top = ui.layout[3];
    ui.handle(
        &WindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(35., top + 100.),
        },
        &mut map,
        &mut p,
    );
    assert_eq!(ui.selected, 1);
    assert!(ui.capture);
    Ok(())
}
