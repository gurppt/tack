//! Public semantic contracts: optional catalog tool and no trail document edits.
use std::time::{Duration, Instant};
use tack_app::{
    actions::{Action, ActionEvent, ActionPhase, Tool},
    image_input::ImageInput,
    local_ui::filtered_actions,
    mouse_tool::MouseTool,
    preferences::Preferences,
    toolbar::{self, Config, Toolbar},
};
use tack_core::{Camera, Document, DocumentEditor, DocumentLimits};
type R = Result<(), Box<dyn std::error::Error + Send + Sync>>;
#[test]
fn optional_mouse_catalog_no_default_shortcut_and_manual_toolbar() -> R {
    let action = Action::SelectTool(Tool::Mouse);
    let profile = Preferences::defaults()?;
    let keymap = profile.keymap()?;
    assert!(filtered_actions("Mulot", &keymap).contains(&action));
    assert_eq!(keymap.for_action(action).count(), 0);
    assert_eq!(
        keymap
            .for_action(Action::TemporaryTool(Tool::Mouse))
            .count(),
        0
    );
    let mut config = Config::default();
    assert!(!config.actions.contains(&action.id()));
    assert!(toolbar::eligible(action));
    config.actions.push(action.id());
    config.normalize()?;
    let mut bar = Toolbar::default();
    bar.layout(&config, [800, 600], 1., true);
    assert!(
        bar.buttons[..bar.count]
            .iter()
            .any(|b| b.action == Some(action))
    );
    assert_eq!(tack_app::toolbar_icons::actual_index(action), Some(36));
    Ok(())
}
#[test]
fn active_mouse_pointer_and_local_effects_leave_document_and_history_unchanged() -> R {
    let now = Instant::now();
    let mut input = ImageInput::new()?;
    let mut e = DocumentEditor::new(
        Document::new(tack_storage::new_document_id()?, DocumentLimits::default()),
        200,
    );
    let mut camera = Camera::new([800, 600]);
    let before = e.document().clone();
    input.dispatch(
        ActionEvent {
            action: Action::SelectTool(Tool::Mouse),
            phase: ActionPhase::Invoke,
        },
        &mut e,
        &mut camera,
    )?;
    input.dispatch(
        ActionEvent {
            action: Action::ImagePointer,
            phase: ActionPhase::Invoke,
        },
        &mut e,
        &mut camera,
    )?;
    let mut mouse = MouseTool::default();
    mouse.activate(true, now);
    mouse.motion([100., 100.], now);
    mouse.motion([250., 250.], now);
    mouse.click([250., 250.], now);
    mouse.draw(&mut input.gizmo, &camera);
    assert!(!input.gizmo.quads.is_empty());
    assert_eq!(*e.document(), before);
    assert_eq!(e.generation(), 0);
    assert_eq!(e.undo_len(), 0);
    mouse.settle(now + Duration::from_secs(1));
    assert_eq!(mouse.effect_count(), 0);
    mouse.activate(false, now + Duration::from_secs(1));
    assert!(mouse.deadline().is_none());
    Ok(())
}
