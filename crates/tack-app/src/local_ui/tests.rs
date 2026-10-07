use super::*;
#[test]
fn scale_and_theme_are_direct_reversible_choices() -> Result<(), tack_assets::AssetError> {
    let mut profile = Preferences::defaults()?;
    let mut keymap = profile.keymap()?;
    let mut ui = LocalUi::new(Panel::Preferences);
    for scale in [2, 1, 4, 0] {
        ui.selected = 3;
        assert!(ui.activate(&mut keymap, &mut profile).is_none());
        assert_eq!(ui.panel, Panel::Scale);
        ui.selected = scale;
        assert!(matches!(
            ui.activate(&mut keymap, &mut profile),
            Some(UiResult::PreferencesChanged)
        ));
        assert_eq!(profile.ui_scale, scale as u8);
        assert_eq!(ui.panel, Panel::Preferences);
    }
    for (index, theme) in crate::ui_theme::Theme::ALL.into_iter().enumerate() {
        ui.selected = 4;
        ui.activate(&mut keymap, &mut profile);
        assert_eq!(ui.panel, Panel::Theme);
        ui.selected = index;
        ui.activate(&mut keymap, &mut profile);
        assert_eq!(profile.theme, theme);
    }
    Ok(())
}
#[test]
fn numeric_changes_are_bounded_and_reversible_without_wrap() -> Result<(), tack_assets::AssetError>
{
    let mut profile = Preferences::defaults()?;
    let mut ui = LocalUi::new(Panel::Preferences);
    for (row, min, max) in [(5, 3, 21), (6, 5, 32)] {
        ui.selected = row;
        for _ in 0..100 {
            ui.adjust(&mut profile, false);
        }
        assert_eq!(
            if row == 5 {
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
            if row == 5 {
                profile.handle_size
            } else {
                profile.hit_radius
            },
            max
        );
        assert!(ui.adjust(&mut profile, true).is_none());
        ui.adjust(&mut profile, false);
        assert_eq!(
            if row == 5 {
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
fn capture_conflict_preserves_old_binding_and_global_reset_requires_confirm()
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
    let before = serde_json::to_vec(&profile)?;
    ui.command(Command::Change, &mut keymap, &mut profile);
    assert!(
        ui.capture_binding(paste.control, &mut keymap, &mut profile)
            .is_none()
    );
    assert_eq!(serde_json::to_vec(&profile)?, before);
    assert!(ui.message.contains(Action::Paste.label()) && ui.message.contains("Refused"));
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
    assert_eq!(filtered_actions("", &keymap).len(), Action::ALL.len());
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
