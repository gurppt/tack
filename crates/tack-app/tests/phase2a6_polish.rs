use std::time::{Duration, Instant};
use tack_app::{
    actions::{Action, ActionEvent, ActionPhase},
    feedback::{Caret, Feedback},
    frame_ui::Label,
    image_gizmo::ImageGizmo,
    image_input::ImageInput,
    input::{PhysicalControl, PhysicalEvent, PointerButton},
    preferences::Preferences,
    toolbar::{Config, Placement, SEPARATOR, Toolbar},
};
use tack_core::*;
use winit::event::{ElementState, MouseButton};
type R = Result<(), Box<dyn std::error::Error + Send + Sync>>;
#[test]
fn compact_cells_and_separator_geometry_on_every_edge_and_scale() -> R {
    let mut cfg = Config {
        actions: vec![
            Action::Save.id(),
            SEPARATOR.into(),
            Action::Undo.id(),
            SEPARATOR.into(),
            SEPARATOR.into(),
            Action::Redo.id(),
        ],
        ..Default::default()
    };
    cfg.normalize()?;
    assert_eq!(cfg.actions.len(), 5);
    let bytes = serde_json::to_vec(&cfg)?;
    assert_eq!(
        serde_json::from_slice::<Config>(&bytes)?.actions,
        cfg.actions
    );
    for placement in Placement::ALL {
        cfg.placement = placement;
        for scale in [1., 2.] {
            let mut bar = Toolbar::default();
            cfg.scale = scale as u8;
            bar.layout(&cfg, [800, 600], scale, false);
            if placement == Placement::Hidden {
                assert_eq!(bar.count, 0);
                continue;
            }
            for b in &bar.buttons[..bar.count] {
                let w = b.rect[2] - b.rect[0];
                let h = b.rect[3] - b.rect[1];
                if b.action.is_some() {
                    assert_eq!([w, h], [16. * scale; 2]);
                } else {
                    assert_eq!(w.min(h), scale);
                    assert_eq!(w.max(h), 16. * scale);
                    assert!(bar.hit([b.rect[0], b.rect[1]]).is_none());
                }
                assert!(b.rect.iter().all(|v| v.fract() == 0.));
            }
            match placement {
                Placement::Top => assert_eq!(bar.bounds[1], 0.),
                Placement::Bottom => assert_eq!(bar.bounds[3], 600.),
                Placement::Left => assert_eq!(bar.bounds[0], 0.),
                Placement::Right => assert_eq!(bar.bounds[2], 800.),
                _ => {}
            }
        }
    }
    Ok(())
}
#[test]
fn acknowledgements_and_carets_drop_deadlines_when_done() {
    let now = Instant::now();
    let mut f = Feedback::default();
    assert!(f.deadline().is_none());
    f.acknowledge(now);
    assert!(f.active());
    assert!(!f.settle(now));
    assert!(f.settle(now + Duration::from_secs(2)));
    assert!(f.deadline().is_none());
    let mut c = Caret::default();
    assert!(c.update(true, now));
    assert!(c.visible);
    assert!(c.update(true, now + Duration::from_millis(600)));
    assert!(!c.visible);
    assert!(c.update(false, now + Duration::from_secs(1)));
    assert!(c.deadline().is_none());
    assert!(!c.update(false, now + Duration::from_secs(2)));
}
#[test]
fn frame_label_double_click_uses_rename_dispatch_without_camera_focus() -> R {
    let id = ObjectId::new(1)?;
    let t = Transform::new([0., 0.], [200., 100.], 0., [false; 2])?;
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddObject {
        object: DocumentObject::frame(id, "Frame".into(), t)?,
        index: 0,
    })?;
    let mut e = DocumentEditor::new(d, 100);
    let mut input = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    c.set_view([0., 0.], 1.)?;
    let label = Label::new(t, &c, 1, "Frame");
    let p = [label.rect[0] + 20., label.rect[1] + 8.];
    let view = c.viewport();
    for _ in 0..2 {
        input.cursor_moved(p, &e, &mut c)?;
        for state in [ElementState::Pressed, ElementState::Released] {
            input.physical(
                PhysicalEvent::Button {
                    control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
                    state,
                    repeat: false,
                },
                &mut e,
                &mut c,
            )?;
        }
    }
    assert_eq!(
        input.name_edit.as_ref().ok_or("double-click failed")?.id,
        id
    );
    assert_eq!(c.viewport(), view);
    input.finish_name_edit(&mut e, false)?;
    input.dispatch(
        ActionEvent {
            action: Action::RenameFrame,
            phase: ActionPhase::Invoke,
        },
        &mut e,
        &mut c,
    )?;
    assert_eq!(input.name_edit.as_ref().ok_or("F2 path")?.id, id);
    assert_eq!(Preferences::defaults()?.frame_title_scale, 1);
    Ok(())
}
#[test]
fn frame_label_palette_and_wrapped_title_hit_geometry_are_readable() -> R {
    let t = Transform::new([0., 0.], [120., 80.], 0., [false; 2])?;
    let mut c = Camera::new([800, 600]);
    c.set_ui_scale(2.);
    let l = Label::new(t, &c, 1, "A deliberately wrapped Frame label");
    assert!(l.rect[3] - l.rect[1] > 36.);
    assert!(l.contains([l.rect[0] + 10., l.rect[1] + 5.]));
    assert!(!l.contains([l.rect[0] + 10., l.rect[3] + 20.]));
    let light = Color([230, 233, 239, 255]);
    let dark = Color([18, 22, 28, 255]);
    let (lb, lt) = tack_app::frame_ui::label_style(light);
    let (db, dt) = tack_app::frame_ui::label_style(dark);
    assert!(lt[0] < lb[0]);
    assert!(dt[0] > db[0]);
    assert_eq!(lb, light.rgba(Opacity::OPAQUE));
    Ok(())
}
#[test]
fn selected_frames_have_only_handles_and_no_gizmo_rectangle() -> R {
    let id = ObjectId::new(1)?;
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddObject {
        object: DocumentObject::frame(
            id,
            "Frame".into(),
            Transform::new([0., 0.], [100., 100.], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    let mut input = ImageInput::new()?;
    input.images.selection.select(Some(id), false);
    let mut g = ImageGizmo::default();
    g.build(&input.images, &d, &Camera::new([800, 600]), None);
    assert!(g.selection.is_empty());
    assert_eq!(g.quads.len(), 8);
    Ok(())
}
#[test]
fn old_default_titles_migrate_but_new_explicit_scale_is_preserved() -> R {
    let root = std::env::temp_dir().join(format!(
        "tack-2a6-profile-{}",
        tack_storage::new_document_id()?.value()
    ));
    std::fs::create_dir(&root)?;
    let result = (|| -> R {
        let p = Preferences::defaults()?;
        let mut old = serde_json::to_value(&p)?;
        old.as_object_mut()
            .ok_or("profile")?
            .remove("frame_title_default_version");
        old["frame_title_scale"] = serde_json::json!(2);
        std::fs::write(root.join("old.json"), serde_json::to_vec(&old)?)?;
        let migrated = tack_app::preferences::read(&root.join("old.json"))?;
        assert_eq!(migrated.frame_title_scale, 1);
        let mut chosen = migrated;
        chosen.frame_title_scale = 2;
        tack_app::preferences::write(&root.join("new.json"), &chosen)?;
        assert_eq!(
            tack_app::preferences::read(&root.join("new.json"))?.frame_title_scale,
            2
        );
        Ok(())
    })();
    std::fs::remove_dir_all(root)?;
    result
}

#[test]
fn frame_hover_is_border_or_title_only_and_blink_keeps_label_geometry() -> R {
    let id = ObjectId::new(1)?;
    let t = Transform::new([0., 0.], [200., 100.], 0., [false; 2])?;
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddObject {
        object: DocumentObject::frame(id, "ABCDEFGHIJKLMNOPQRSTUVWX".into(), t)?,
        index: 0,
    })?;
    let mut e = DocumentEditor::new(d, 100);
    let mut input = ImageInput::new()?;
    let mut c = Camera::new([800, 600]);
    c.set_view([0., 0.], 1.)?;
    input.cursor_moved([400., 300.], &e, &mut c)?;
    input.build_overlay(&e, &c);
    let normal = input.gizmo.quads[0].color;
    input.cursor_moved([300., 300.], &e, &mut c)?;
    input.build_overlay(&e, &c);
    assert_ne!(input.gizmo.quads[0].color, normal);
    let label = Label::new(t, &c, 1, "ABCDEFGHIJKLMNOPQRSTUVWX");
    input.cursor_moved([label.rect[0] + 10., label.rect[1] + 3.], &e, &mut c)?;
    input.build_overlay(&e, &c);
    assert_ne!(input.gizmo.quads[0].color, normal);
    input.images.selection.select(Some(id), false);
    input.dispatch(
        ActionEvent {
            action: Action::RenameFrame,
            phase: ActionPhase::Invoke,
        },
        &mut e,
        &mut c,
    )?;
    input.caret.visible = true;
    input.build_overlay(&e, &c);
    let shown = input.gizmo.quads.clone();
    input.caret.visible = false;
    input.build_overlay(&e, &c);
    assert_eq!(shown.len(), input.gizmo.quads.len() + 1);
    for (a, b) in shown.iter().zip(&input.gizmo.quads) {
        assert_eq!(a.points, b.points);
        assert_eq!(a.bitmap, b.bitmap);
    }
    Ok(())
}
