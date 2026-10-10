use super::*;
#[test]
fn clock_cancellation_one_shot_and_reactivation() {
    let now = Instant::now();
    let mut mouse = MouseTool::default();
    assert!(mouse.deadline().is_none());
    mouse.activate(true, now);
    assert_eq!(mouse.deadline(), Some(now + EASTER_WAIT));
    assert_eq!(
        mouse.settle(now + EASTER_WAIT - Duration::from_nanos(1)),
        (false, false)
    );
    mouse.activate(false, now + EASTER_WAIT - Duration::from_nanos(1));
    assert!(mouse.deadline().is_none());
    assert!(!mouse.settle(now + EASTER_WAIT).1);
    mouse.activate(true, now + EASTER_WAIT);
    assert!(mouse.settle(now + EASTER_WAIT * 2).1);
    assert!(mouse.deadline().is_none());
    assert!(!mouse.settle(now + EASTER_WAIT * 10).1);
    mouse.activate(true, now + EASTER_WAIT * 10);
    assert!(mouse.deadline().is_none());
    mouse.activate(false, now + EASTER_WAIT * 10);
    mouse.activate(true, now + EASTER_WAIT * 10);
    assert!(mouse.settle(now + EASTER_WAIT * 11).1);
    assert_eq!(mouse.spawns, 2);
}
#[test]
fn direction_mapping_gait_minimum_travel_bounds_and_expiry() {
    let now = Instant::now();
    for (i, d) in [
        [0., -1.],
        [1., -1.],
        [1., 0.],
        [1., 1.],
        [0., 1.],
        [-1., 1.],
        [-1., 0.],
        [-1., -1.],
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(direction(d), i);
    }
    let mut m = MouseTool::default();
    m.activate(true, now);
    assert!(!m.motion([0.; 2], now));
    assert!(!m.motion([17., 0.], now));
    assert!(m.motion([18., 0.], now));
    assert!(!m.motion([18., 0.], now));
    assert!(m.motion([36., 0.], now));
    let feet: Vec<_> = m
        .effects
        .iter()
        .filter_map(|e| match e {
            Effect::Paw { at, sprite, .. } => Some((*at, *sprite)),
            _ => None,
        })
        .collect();
    assert_eq!(feet, vec![([10., -11.], 4), ([28., -5.], 5)]);
    m.motion([1e8, 0.], now);
    assert_eq!(m.effect_count(), LIMIT);
    assert!(m.click([100., 100.], now));
    assert_eq!(m.effect_count(), LIMIT);
    assert!(m.settle(now + PING_STEP).0);
    assert_eq!(
        m.effects.back().map(|e| match e {
            Effect::Ping { stage, .. } => *stage,
            _ => 99,
        }),
        Some(1)
    );
    assert!(m.settle(now + PING_STEP * 3).0);
    assert!(m.effects.iter().all(|e| matches!(e, Effect::Paw { .. })));
    assert!(m.settle(now + PAW_LIFETIME).0);
    assert_eq!(m.effect_count(), 0);
    assert_eq!(m.deadline(), Some(now + EASTER_WAIT));
    m.activate(false, now + PAW_LIFETIME);
    assert!(m.deadline().is_none());
}
#[test]
fn authored_masks_flip_without_new_artwork() -> Result<(), AssetError> {
    let path = crate::toolbar_icons::root().join("pawL_45.png");
    let base = load_sprite(&path, false, false)?;
    let mirrored = load_sprite(&path, true, true)?;
    assert!(base.bits.iter().any(|v| *v != 0));
    assert_eq!(base.color, mirrored.color);
    assert_eq!(
        base.bits.iter().map(|v| v.count_ones()).sum::<u32>(),
        mirrored.bits.iter().map(|v| v.count_ones()).sum::<u32>()
    );
    Ok(())
}

#[test]
fn mouse_activation_cancels_drag_and_finishes_active_scribble_before_switch()
-> Result<(), AssetError> {
    use crate::actions::{Action, ActionEvent, ActionPhase, HoldToken, Tool};
    use tack_core::*;
    for temporary in [false, true] {
        let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
        d.apply(Command::AddSource(Source::embedded(SourceId::new(2)?)))?;
        d.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(3)?,
            SourceId::new(2)?,
            [100; 2],
        )?))?;
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(4)?,
                AssetId::new(3)?,
                Transform::new([0.; 2], [100.; 2], 0., [false; 2])?,
            ),
            index: 0,
        })?;
        let mut e = DocumentEditor::new(d, 200);
        let before = e.document().clone();
        let mut c = Camera::new([800, 600]);
        let mut i = crate::image_input::ImageInput::new()?;
        i.cursor_moved([400., 300.], &e, &mut c)?;
        let button = |state| crate::input::PhysicalEvent::Button {
            control: crate::input::PhysicalControl::Pointer(crate::input::PointerButton::Mouse(
                winit::event::MouseButton::Left,
            )),
            state,
            repeat: false,
        };
        i.physical(button(winit::event::ElementState::Pressed), &mut e, &mut c)?;
        i.cursor_moved([420., 320.], &e, &mut c)?;
        assert!(i.images.active());
        let event = if temporary {
            ActionEvent {
                action: Action::TemporaryTool(Tool::Mouse),
                phase: ActionPhase::Begin(HoldToken(20)),
            }
        } else {
            ActionEvent {
                action: Action::SelectTool(Tool::Mouse),
                phase: ActionPhase::Invoke,
            }
        };
        i.dispatch(event, &mut e, &mut c)?;
        i.physical(button(winit::event::ElementState::Released), &mut e, &mut c)?;
        i.cursor_moved([460., 360.], &e, &mut c)?;
        assert!(!i.active());
        assert_eq!(*e.document(), before);
        if temporary {
            i.dispatch(
                ActionEvent {
                    action: event.action,
                    phase: ActionPhase::End(HoldToken(20)),
                },
                &mut e,
                &mut c,
            )?;
        }
        i.dispatch(
            ActionEvent {
                action: Action::SelectTool(Tool::Scribble),
                phase: ActionPhase::Invoke,
            },
            &mut e,
            &mut c,
        )?;
        i.cursor_moved([100., 100.], &e, &mut c)?;
        i.physical(button(winit::event::ElementState::Pressed), &mut e, &mut c)?;
        i.cursor_moved([200., 100.], &e, &mut c)?;
        i.dispatch(event, &mut e, &mut c)?;
        assert_eq!(e.document().annotation_count(), 1);
        assert_eq!(e.undo_len(), 1);
        assert!(i.annotation.creation.is_none());
    }
    Ok(())
}
