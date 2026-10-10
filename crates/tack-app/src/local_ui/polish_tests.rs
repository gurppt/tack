use super::*;
type R = Result<(), tack_assets::AssetError>;
#[test]
fn fixed_menu_capture_is_refused_and_safe_confirmations_block_escape_hatch() -> R {
    let mut profile = Preferences::defaults()?;
    let mut map = profile.keymap()?;
    let original = profile.clone();
    let mut ui = LocalUi::new(Panel::Keymap);
    ui.capture = true;
    ui.capture_binding(
        PhysicalControl::LogicalKey(crate::input::LogicalKey::Named(
            winit::keyboard::NamedKey::F10,
        )),
        &mut map,
        &mut profile,
    );
    assert!(ui.staged.is_none());
    assert!(ui.message.contains("reserved"));
    assert_eq!(profile, original);
    assert!(!ui.blocks_menu_access());
    ui.confirm_reset = Some(ResetScope::All);
    assert!(ui.blocks_menu_access());
    for panel in [Panel::Close, Panel::Recovery, Panel::Connecting] {
        assert!(LocalUi::new(panel).blocks_menu_access());
    }
    Ok(())
}
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
#[test]
fn scrollbar_wheel_drag_and_hover_never_retarget_or_scroll_from_pointer_position() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let c = Camera::new([800, 600]);
    let mut g = ImageGizmo::default();
    for panel in [Panel::Keymap, Panel::Toolbar] {
        let mut ui = LocalUi::new(panel);
        ui.draw(&mut g, &c, &map, &p);
        let motion = |x, y| WindowEvent::CursorMoved {
            device_id: winit::event::DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(x, y),
        };
        let wheel = WindowEvent::MouseWheel {
            device_id: winit::event::DeviceId::dummy(),
            delta: winit::event::MouseScrollDelta::LineDelta(0., -1.),
            phase: winit::event::TouchPhase::Moved,
        };
        for y in [85., 86., 100., 200., 300.] {
            ui.handle(&motion(50., y), &mut map, &mut p);
            g.quads.clear();
            ui.draw(&mut g, &c, &map, &p);
            assert_eq!(ui.first, 0);
            assert_eq!(ui.selected, 0);
        }
        ui.handle(&motion(50., 100.), &mut map, &mut p);
        ui.handle(&wheel, &mut map, &mut p);
        g.quads.clear();
        ui.draw(&mut g, &c, &map, &p);
        assert_eq!(ui.first, 1);
        assert_eq!(ui.selected, 0);
        let viewport = ui.scroll[0].viewport;
        ui.handle(
            &motion(viewport[2] - 4., viewport[1] + 4.),
            &mut map,
            &mut p,
        );
        ui.handle(
            &WindowEvent::MouseInput {
                device_id: winit::event::DeviceId::dummy(),
                state: ElementState::Pressed,
                button: winit::event::MouseButton::Left,
            },
            &mut map,
            &mut p,
        );
        ui.handle(
            &motion(viewport[2] - 4., viewport[3] - 1.),
            &mut map,
            &mut p,
        );
        assert!(ui.first > 1);
        ui.modifiers = Modifiers::CONTROL;
        ui.focus = Some(Command::Search);
        ui.handle(&WindowEvent::Focused(false), &mut map, &mut p);
        assert_eq!(ui.modifiers, Modifiers::NONE);
        assert!(ui.focus.is_none());
    }
    Ok(())
}
#[test]
fn capture_is_staged_cancelable_and_clears_refused_candidates() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let before = map.bindings().to_vec();
    let mut ui = LocalUi::new(Panel::Keymap);
    ui.search = "Undo".into();
    ui.command(Command::Change, &mut map, &mut p);
    ui.modifiers = Modifiers::CONTROL;
    ui.capture_binding(
        PhysicalControl::LogicalKey(crate::input::LogicalKey::Character('v')),
        &mut map,
        &mut p,
    );
    assert!(ui.staged.is_some());
    assert_eq!(map.bindings(), before);
    assert!(ui.message.contains("Paste"));
    ui.capture_binding(
        PhysicalControl::Wheel(crate::input::WheelAxis::Vertical),
        &mut map,
        &mut p,
    );
    assert!(ui.staged.is_none());
    assert_eq!(map.bindings(), before);
    ui.command(Command::Cancel, &mut map, &mut p);
    assert!(!ui.capture);
    assert_eq!(map.bindings(), before);
    Ok(())
}
#[test]
fn release_behavior_is_real_and_captured_actions_refuse_illegal_modes() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let mut ui = LocalUi::new(Panel::Keymap);
    ui.search = "Undo".into();
    ui.command(Command::Trigger, &mut map, &mut p);
    assert_eq!(
        map.for_action(Action::Undo).next().ok_or("undo")?.trigger,
        Trigger::Release
    );
    ui.command(Command::Trigger, &mut map, &mut p);
    assert_eq!(
        map.for_action(Action::Undo).next().ok_or("undo")?.trigger,
        Trigger::Press
    );
    ui.search = "SnapDisable".into();
    ui.selected = 0;
    ui.command(Command::Trigger, &mut map, &mut p);
    assert_eq!(
        map.for_action(Action::SnapDisable)
            .next()
            .ok_or("snap")?
            .trigger,
        Trigger::Hold
    );
    ui.search = "ZoomView".into();
    ui.command(Command::Trigger, &mut map, &mut p);
    assert!(ui.message.contains("Normal"));
    Ok(())
}
#[test]
fn bookmark_capture_displaces_release_conflict_and_returns_only_after_confirmation() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = Keymap::default();
    let control = PhysicalControl::LogicalKey(crate::input::LogicalKey::Named(
        winit::keyboard::NamedKey::F4,
    ));
    map.bind(Binding {
        action: Action::Undo,
        control,
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Release,
    })?;
    let mut ui = LocalUi::view_capture(11);
    assert!(ui.capture_binding(control, &mut map, &mut p).is_none());
    assert!(ui.message.contains("Undo"));
    assert_eq!(map.for_action(Action::Undo).count(), 1);
    assert!(matches!(
        ui.command(Command::ConfirmCapture, &mut map, &mut p),
        Some(UiResult::AssignCameraSlot(11))
    ));
    assert_eq!(map.for_action(Action::Undo).count(), 0);
    assert_eq!(map.for_action(Action::JumpCameraSlot(11)).count(), 1);
    assert_eq!(
        Action::from_id("JumpCameraSlot(11)"),
        Some(Action::JumpCameraSlot(11))
    );
    assert!(Action::from_id("JumpCameraSlot(64)").is_none());
    assert!(filtered_actions("F4", &map).contains(&Action::JumpCameraSlot(11)));
    Ok(())
}

#[test]
fn bookmark_focus_loss_dismisses_and_capacity_race_preserves_staged_binding() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let mut ui = LocalUi::view_capture(12);
    assert!(matches!(
        ui.handle(&WindowEvent::Focused(false), &mut map, &mut p),
        Some(UiResult::Dismiss)
    ));
    let mut ui = LocalUi::view_capture(12);
    ui.capture_binding(
        PhysicalControl::LogicalKey(crate::input::LogicalKey::Character('q')),
        &mut map,
        &mut p,
    );
    let before = map.bindings().to_vec();
    p.local_views = (0..64)
        .map(|slot| crate::camera_slots::View {
            board: format!("{:032x}", 1),
            slot,
            center: [0.; 2],
            zoom: 1.,
        })
        .collect();
    assert!(
        ui.command(Command::ConfirmCapture, &mut map, &mut p)
            .is_none()
    );
    assert_eq!(map.bindings(), before);
    assert!(ui.message.contains("64"));
    Ok(())
}
#[test]
fn update_metadata_rows_cannot_download_or_restart() -> R {
    let mut p = Preferences::defaults()?;
    let mut map = p.keymap()?;
    let c = Camera::new([800, 600]);
    for panel in [Panel::UpdateOffer, Panel::UpdateReady] {
        let mut ui = LocalUi::daily(
            panel,
            DailyPanel {
                rows: vec!["Current: old".into(), "Available: new".into()],
                ..Default::default()
            },
        );
        let mut g = ImageGizmo::default();
        ui.draw(&mut g, &c, &map, &p);
        let offset = ui.panel_offset;
        ui.cursor = [
            (40. + offset[0]) * ui.layout[0],
            (ui.layout[3] + 5. + offset[1]) * ui.layout[0],
        ];
        assert!(
            ui.handle(
                &WindowEvent::MouseInput {
                    device_id: winit::event::DeviceId::dummy(),
                    state: ElementState::Pressed,
                    button: winit::event::MouseButton::Left
                },
                &mut map,
                &mut p
            )
            .is_none()
        );
        assert!(matches!(
            ui.command(Command::ConfirmDaily, &mut map, &mut p),
            Some(UiResult::UpdateAdvance)
        ));
    }
    let mut ui = LocalUi::new(Panel::UpdateApplying);
    assert!(ui.blocks_menu_access());
    assert!(
        ui.handle(&WindowEvent::Focused(false), &mut map, &mut p)
            .is_none()
    );
    Ok(())
}
