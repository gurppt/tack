use super::*;
#[test]
fn themes_are_inline_live_persistent_and_never_instantiate_modal()
-> Result<(), tack_assets::AssetError> {
    let mut profile = Preferences::defaults()?;
    let mut keymap = profile.keymap()?;
    let mut ui = LocalUi::new(Panel::Preferences);
    let camera = Camera::new([800, 600]);
    let mut gizmo = ImageGizmo::default();
    for (index, theme) in crate::ui_theme::Theme::ALL.into_iter().enumerate() {
        ui.selected = index + 3;
        assert!(matches!(
            ui.activate(&mut keymap, &mut profile),
            Some(UiResult::PreferencesChanged)
        ));
        ui.draw(&mut gizmo, &camera, &keymap, &profile);
        assert_eq!(profile.theme, theme);
        assert_eq!(ui.panel, Panel::Preferences);
        assert!(ui.modal.is_none());
        assert!(ui.parents.is_empty());
        let persisted: Preferences = serde_json::from_slice(&serde_json::to_vec(&profile)?)?;
        assert_eq!(persisted.theme, theme);
        gizmo.quads.clear();
    }
    assert!(matches!(ui.back(), Some(UiResult::Dismiss)));
    Ok(())
}
#[test]
fn nested_keymap_back_and_direct_close_release_all_capture_state()
-> Result<(), tack_assets::AssetError> {
    let mut profile = Preferences::defaults()?;
    let mut keymap = profile.keymap()?;
    let mut ui = LocalUi::new(Panel::Keymap);
    ui.capture = true;
    ui.behavior = crate::shortcut_capture::Behavior::Hold;
    ui.focus = Some(Command::Search);
    ui.confirm_reset = Some(ResetScope::All);
    assert!(matches!(
        ui.command(Command::Close, &mut keymap, &mut profile),
        Some(UiResult::Dismiss)
    ));
    assert!(
        !ui.capture
            && ui.behavior == crate::shortcut_capture::Behavior::Normal
            && ui.focus.is_none()
            && ui.confirm_reset.is_none()
    );
    let mut direct = LocalUi::new(Panel::Keymap);
    assert!(matches!(direct.back(), Some(UiResult::Dismiss)));
    Ok(())
}
#[test]
fn numeric_changes_are_bounded_and_reversible_without_wrap() -> Result<(), tack_assets::AssetError>
{
    let mut profile = Preferences::defaults()?;
    let mut ui = LocalUi::new(Panel::Preferences);
    for (row, min, max) in [(6, 3, 21), (7, 5, 32)] {
        ui.selected = row;
        for _ in 0..100 {
            ui.adjust(&mut profile, false);
        }
        assert_eq!(
            if row == 6 {
                profile.handle_size
            } else {
                profile.hit_radius
            },
            min
        );
        assert!(ui.adjust(&mut profile, false).is_none());
        for _ in 0..100 {
            ui.adjust(&mut profile, true);
        }
        assert_eq!(
            if row == 6 {
                profile.handle_size
            } else {
                profile.hit_radius
            },
            max
        );
        assert!(ui.adjust(&mut profile, true).is_none());
        ui.adjust(&mut profile, false);
        assert_eq!(
            if row == 6 {
                profile.handle_size
            } else {
                profile.hit_radius
            },
            max - 1
        );
    }
    profile.keymap()?;
    Ok(())
}
#[test]
fn capture_conflict_reassigns_binding_and_global_reset_requires_confirm()
-> Result<(), tack_assets::AssetError> {
    let mut profile = Preferences::defaults()?;
    let mut keymap = profile.keymap()?;
    let mut ui = LocalUi::new(Panel::Keymap);
    ui.search = "Undo".into();
    let paste = *keymap
        .for_action(Action::Paste)
        .next()
        .ok_or("Paste binding")?;
    ui.modifiers = match paste.modifiers {
        ModifierMatch::Exact(m) => m,
        _ => Modifiers::NONE,
    };
    ui.command(Command::Change, &mut keymap, &mut profile);
    assert!(
        ui.capture_binding(paste.control, &mut keymap, &mut profile)
            .is_none()
    );
    assert_eq!(keymap.for_action(Action::Paste).count(), 1);
    assert!(ui.message.contains(Action::Paste.label()));
    assert!(matches!(
        ui.command(Command::ConfirmCapture, &mut keymap, &mut profile),
        Some(UiResult::PreferencesChanged)
    ));
    assert_eq!(keymap.for_action(Action::Paste).count(), 0);
    assert_eq!(
        keymap
            .for_action(Action::Undo)
            .next()
            .ok_or("Undo")?
            .control,
        paste.control
    );
    assert!(ui.message.contains(Action::Paste.label()) && ui.message.contains("replaces"));
    ui.command(Command::Cancel, &mut keymap, &mut profile);
    ui.command(Command::Unassign, &mut keymap, &mut profile);
    assert_eq!(keymap.for_action(Action::Undo).count(), 0);
    ui.command(Command::ResetAll, &mut keymap, &mut profile);
    assert_eq!(keymap.for_action(Action::Undo).count(), 0);
    ui.command(Command::Cancel, &mut keymap, &mut profile);
    assert_eq!(keymap.for_action(Action::Undo).count(), 0);
    ui.command(Command::ResetAll, &mut keymap, &mut profile);
    ui.command(Command::ConfirmReset, &mut keymap, &mut profile);
    assert!(keymap.for_action(Action::Undo).count() > 0);
    Ok(())
}
#[test]
fn filter_matches_action_category_and_active_shortcut_case_insensitively()
-> Result<(), tack_assets::AssetError> {
    let profile = Preferences::defaults()?;
    let keymap = profile.keymap()?;
    assert_eq!(
        filtered_actions("", &keymap).len(),
        Action::ALL
            .into_iter()
            .filter(|a| !matches!(a, Action::TemporaryTool(_)))
            .count()
    );
    assert!(filtered_actions("uNdO", &keymap).contains(&Action::Undo));
    assert!(filtered_actions(Action::Undo.category(), &keymap).contains(&Action::Undo));
    let label = crate::context_menu::binding_label(
        keymap
            .for_action(Action::Undo)
            .next()
            .ok_or("Undo binding")?,
    );
    assert!(filtered_actions(&label.to_lowercase(), &keymap).contains(&Action::Undo));
    assert!(filtered_actions("no-such-action-123", &keymap).is_empty());
    Ok(())
}
