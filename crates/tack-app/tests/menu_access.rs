use tack_app::{
    actions::Action,
    bindings::{Binding, BindingError, Keymap, ModifierMatch, Trigger},
    input::{LogicalKey, Modifiers, PhysicalControl},
    preferences::{BindingRecord, Preferences},
};
use winit::keyboard::{KeyCode, NamedKey, PhysicalKey};
type R = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[test]
fn menu_key_cannot_be_reassigned_or_changed_to_release_or_hold() -> R {
    for control in [
        PhysicalControl::Key(PhysicalKey::Code(KeyCode::F10)),
        PhysicalControl::LogicalKey(LogicalKey::Named(NamedKey::F10)),
    ] {
        for trigger in [Trigger::Press, Trigger::Release, Trigger::Hold] {
            let mut map = Keymap::default();
            assert_eq!(
                map.bind(Binding {
                    action: Action::About,
                    control,
                    modifiers: ModifierMatch::Exact(Modifiers::NONE),
                    trigger,
                }),
                Err(BindingError::ReservedMenu)
            );
            assert!(map.bindings().is_empty());
        }
        let mut map = Keymap::default();
        let menu = Binding {
            action: Action::ApplicationMenu,
            control,
            modifiers: ModifierMatch::Exact(Modifiers::NONE),
            trigger: Trigger::Press,
        };
        map.bind(menu)?;
        map.bind(Binding {
            action: Action::About,
            modifiers: ModifierMatch::Exact(Modifiers::CONTROL),
            ..menu
        })?;
    }
    Ok(())
}

#[test]
fn legacy_menu_override_is_inactive_without_losing_other_preferences() -> R {
    let mut profile = Preferences::defaults()?;
    profile.keymap.retain(|b| b.action != "ApplicationMenu");
    profile.keymap.push(BindingRecord::from_binding(&Binding {
        action: Action::About,
        control: PhysicalControl::LogicalKey(LogicalKey::Named(NamedKey::F10)),
        modifiers: ModifierMatch::Exact(Modifiers::NONE),
        trigger: Trigger::Release,
    }));
    profile.toolbar.scale = 3;
    profile.ui_scale = 2;
    let original = profile.clone();
    let map = profile.keymap()?;
    assert!(!map.bindings().iter().any(tack_app::menu_access::reserved));
    assert!(map.for_action(Action::Save).next().is_some());
    assert_eq!(profile, original);
    let parsed: Preferences = serde_json::from_slice(&serde_json::to_vec(&profile)?)?;
    parsed.keymap()?;
    assert_eq!(parsed, original);
    Ok(())
}
