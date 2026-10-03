use tack_app::{
    actions::{Action, ActionEvent, ActionPhase},
    image_geometry as geometry,
    image_gizmo::{GizmoHit, ImageGizmo},
    image_input::ImageInput,
    image_interaction::{GestureKind, ImageInteraction},
    input::{Modifiers, PhysicalControl, PhysicalEvent, PointerButton},
};
use tack_core::*;
use winit::event::{ElementState, MouseButton};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn fixture() -> Result<DocumentEditor> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [6000, 4500],
    )?))?;
    for id in 1..=3 {
        doc.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(id)?,
                AssetId::new(1)?,
                Transform::new(
                    [if id < 3 { 0. } else { 200. }, 0.],
                    [100., 80.],
                    if id == 2 { 0.3 } else { 0. },
                    [id == 2, false],
                )?,
            ),
            index: doc.object_order().len(),
        })?;
    }
    Ok(DocumentEditor::new(doc, 200))
}
fn button(state: ElementState) -> PhysicalEvent {
    PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Left)),
        state,
        repeat: false,
    }
}
#[test]
fn selection_topmost_transformed_quad_and_marquee_do_not_touch_document() -> Result {
    let e = fixture()?;
    let before = e.document().clone();
    let mut images = ImageInteraction::default();
    assert_eq!(images.hit(e.document(), [0., 0.]), Some(ObjectId::new(2)?));
    assert_eq!(images.hit(e.document(), [f64::NAN, 0.]), None);
    images
        .selection
        .select(images.hit(e.document(), [0., 0.]), false);
    images.selection.select(Some(ObjectId::new(3)?), true);
    assert_eq!(images.selection.len(), 2);
    images.selection.select(Some(ObjectId::new(2)?), true);
    assert_eq!(images.selection.len(), 1);
    images.selection.select(None, false);
    assert!(images.selection.is_empty());
    images
        .selection
        .marquee(e.document(), WorldRect::new(-5., -5., 10., 10.)?, false);
    assert_eq!(images.selection.len(), 2);
    assert_eq!(e.document(), &before);
    let t = Transform::new(
        [0.; 2],
        [100., 20.],
        std::f64::consts::FRAC_PI_4,
        [true, true],
    )?;
    assert!(geometry::hit(t, geometry::world(t, [40., 8.])));
    assert!(!geometry::hit(t, geometry::world(t, [40., 15.])));
    assert!(!geometry::intersects(t, WorldRect::new(30., -30., 2., 2.)?));
    Ok(())
}
#[test]
fn every_gesture_preview_is_transient_commit_has_one_exact_inverse() -> Result {
    for kind in [
        GestureKind::Move,
        GestureKind::Resize {
            handle: 4,
            center: false,
        },
        GestureKind::Scale,
        GestureKind::Rotate,
        GestureKind::Crop { handle: 3 },
        GestureKind::Opacity,
    ] {
        let mut e = fixture()?;
        let before = e.document().clone();
        let mut images = ImageInteraction::default();
        images.selection.select(Some(ObjectId::new(2)?), false);
        images.begin(kind, [40., 0.], &e)?;
        for i in 1..=300 {
            images.update([40. + i as f64 * 0.04, i as f64 * 0.02])?;
        }
        assert_eq!(e.document(), &before);
        assert_eq!(e.undo_len(), 0);
        images.cancel();
        assert_eq!(e.document(), &before);
        images.begin(kind, [40., 0.], &e)?;
        images.update([15., 18.])?;
        assert!(images.commit(&mut e)?);
        assert_eq!(e.undo_len(), 1);
        let after = e.document().clone();
        e.undo()?;
        assert_eq!(e.document(), &before);
        e.redo()?;
        assert_eq!(e.document(), &after);
        assert!(!images.commit(&mut e)?);
    }
    Ok(())
}
#[test]
fn multi_move_resize_rotate_preserve_spacing_and_atomic_history() -> Result {
    for kind in [
        GestureKind::Move,
        GestureKind::Resize {
            handle: 4,
            center: false,
        },
        GestureKind::Rotate,
    ] {
        let mut e = fixture()?;
        let before = e.document().clone();
        let mut images = ImageInteraction::default();
        images.selection.select_all(e.document());
        images.begin(kind, [300., 150.], &e)?;
        images.update([360., 190.])?;
        images.commit(&mut e)?;
        assert_eq!(e.undo_len(), 1);
        let after = e.document().clone();
        let a = e
            .document()
            .object(ObjectId::new(1)?)
            .ok_or("object")?
            .transform();
        let b = e
            .document()
            .object(ObjectId::new(3)?)
            .ok_or("object")?
            .transform();
        let distance = (a.center()[0] - b.center()[0]).hypot(a.center()[1] - b.center()[1]);
        let scale = if matches!(kind, GestureKind::Resize { .. }) {
            a.size()[0] / 100.
        } else {
            1.
        };
        assert!((distance - 200. * scale).abs() < 1e-9);
        e.undo()?;
        assert_eq!(e.document(), &before);
        e.redo()?;
        assert_eq!(e.document(), &after);
    }
    Ok(())
}
fn sample_world(data: ImageRenderData, uv: [f64; 2]) -> [f64; 2] {
    let crop = data.crop.uv_rect();
    let t = data.transform;
    let mut fraction = [(uv[0] - crop[0]) / crop[2], (uv[1] - crop[1]) / crop[3]];
    for (i, f) in fraction.iter_mut().enumerate() {
        if t.flips()[i] {
            *f = 1. - *f;
        }
    }
    geometry::world(
        t,
        [
            (fraction[0] - 0.5) * t.size()[0],
            (fraction[1] - 0.5) * t.size()[1],
        ],
    )
}
#[test]
fn randomized_rotated_flipped_crop_preserves_retained_pixel_mapping_and_opposite_edge() -> Result {
    let e = fixture()?;
    let base = e
        .document()
        .object_render_data(ObjectId::new(1)?)
        .ok_or("render data")?;
    for n in 0..400 {
        let rotation = n as f64 * 0.143;
        let t = Transform::new(
            [n as f64, -200.],
            [100., 80.],
            rotation,
            [n % 2 == 0, n % 3 == 0],
        )?;
        let data = ImageRenderData {
            transform: t,
            ..base
        };
        let delta = geometry::rotate([-20., -10.], rotation);
        let after = geometry::crop(data, delta, [1., 1.])?;
        let p = sample_world(data, [0.5, 0.5]);
        let q = sample_world(after, [0.5, 0.5]);
        assert!((p[0] - q[0]).hypot(p[1] - q[1]) < 1e-10);
        let p = geometry::world(t, [-50., -40.]);
        let q = geometry::world(after.transform, after.transform.size().map(|s| -s / 2.));
        assert!((p[0] - q[0]).hypot(p[1] - q[1]) < 1e-10);
        let restored = geometry::crop(after, geometry::rotate([20., 10.], rotation), [1., 1.])?;
        assert!((restored.crop.uv_rect()[2] - 1.).abs() < 1e-12);
    }
    Ok(())
}
#[test]
fn local_resize_pivot_and_screen_space_handles_are_dpi_zoom_independent() -> Result {
    for scale in [1., 1.25, 1.5, 2.] {
        for zoom in [0.001, 0.1, 1., 64.] {
            let t = Transform::new([100., 80.], [400., 300.], 0.7, [true, false])?;
            let mut camera = Camera::new([1280, 720]);
            camera.set_view(t.center(), zoom)?;
            let mut gizmo = ImageGizmo::default();
            gizmo.set_scale(scale);
            let point = camera.world_to_screen(gizmo.handle(t, &camera, 4));
            assert!(
                gizmo
                    .hit(t, &camera, [point[0] + 8. * scale, point[1]], false, false)
                    .is_some()
            );
            assert_eq!(
                gizmo.hit(t, &camera, point, false, false),
                Some(GizmoHit::Resize(4))
            );
            assert!(
                gizmo
                    .hit(
                        t,
                        &camera,
                        [point[0] + 10. * scale, point[1] + 10. * scale],
                        false,
                        false
                    )
                    .is_none()
            );
            let resized = geometry::resize(
                t,
                geometry::rotate([80., 20.], t.rotation()),
                [1., 1.],
                false,
                false,
            )?;
            let before = geometry::world(t, t.size().map(|s| -s / 2.));
            let after = geometry::world(resized, resized.size().map(|s| -s / 2.));
            assert!((before[0] - after[0]).hypot(before[1] - after[1]) < 1e-10);
            let centered = geometry::resize(t, [80., 20.], [1., 1.], true, false)?;
            assert_eq!(centered.center(), t.center());
            let screen = camera.world_to_screen([101.125, 77.375]);
            let world = camera.screen_to_world(screen);
            assert!((world[0] - 101.125).abs() < 1e-9);
        }
    }
    Ok(())
}
#[test]
fn semantic_input_release_remap_cancel_focus_and_late_alt_cleanup() -> Result {
    for cancellation in 0..4 {
        let mut e = fixture()?;
        let before = e.document().clone();
        let mut input = ImageInput::new()?;
        let mut camera = Camera::new([1280, 720]);
        input.cursor_moved([640., 360.], &e, &mut camera)?;
        input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
        input.cursor_moved([665., 380.], &e, &mut camera)?;
        assert!(input.images.active());
        assert_eq!(e.document(), &before);
        match cancellation {
            0 => {
                input.dispatch(
                    ActionEvent {
                        action: Action::CancelInteraction,
                        phase: ActionPhase::Invoke,
                    },
                    &mut e,
                    &mut camera,
                )?;
            }
            1 => {
                input.physical(PhysicalEvent::FocusLost, &mut e, &mut camera)?;
            }
            2 => input
                .handle(
                    &winit::event::WindowEvent::CursorLeft {
                        device_id: winit::event::DeviceId::dummy(),
                    },
                    &mut e,
                    &mut camera,
                )
                .map(|_| ())?,
            _ => {
                input.physical(
                    PhysicalEvent::Modifiers(Modifiers::ALT),
                    &mut e,
                    &mut camera,
                )?;
                input.cursor_moved([680., 390.], &e, &mut camera)?;
            }
        }
        input.physical(button(ElementState::Released), &mut e, &mut camera)?;
        assert_eq!(e.document(), &before);
        assert!(!input.active());
        input.physical(PhysicalEvent::FocusLost, &mut e, &mut camera)?;
    }
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input.cursor_moved([640., 360.], &e, &mut camera)?;
    input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
    input.cursor_moved([700., 360.], &e, &mut camera)?;
    input.keymap.unassign(Action::ImagePointer);
    input.physical(button(ElementState::Released), &mut e, &mut camera)?;
    assert_eq!(e.undo_len(), 1);
    assert!(!input.active());
    Ok(())
}
#[test]
fn invalid_preview_and_external_document_change_reject_safely() -> Result {
    let mut e = fixture()?;
    let mut images = ImageInteraction::default();
    let id = ObjectId::new(1)?;
    images.selection.select(Some(id), false);
    images.begin(GestureKind::Move, [0.; 2], &e)?;
    images.update([10., 20.])?;
    let preview = images.preview(e.document().object_render_data(id).ok_or("data")?);
    assert!(images.update([f64::NAN, 0.]).is_err());
    assert!(images.update([1e10, 0.]).is_err());
    assert_eq!(
        images.preview(e.document().object_render_data(id).ok_or("data")?),
        preview
    );
    e.execute(Command::SetOpacity {
        object: id,
        opacity: Opacity::new(0.5)?,
    })?;
    let before = e.document().clone();
    assert!(images.commit(&mut e).is_err());
    assert_eq!(e.document(), &before);
    Ok(())
}

#[test]
fn alt_middle_over_handle_always_pans_and_alt_left_resizes_about_center() -> Result {
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input
        .images
        .selection
        .select(Some(ObjectId::new(2)?), false);
    let t = e
        .document()
        .object(ObjectId::new(2)?)
        .ok_or("object")?
        .transform();
    let point = camera.world_to_screen(input.gizmo.handle(t, &camera, 4));
    input.cursor_moved(point, &e, &mut camera)?;
    input.physical(
        PhysicalEvent::Modifiers(Modifiers::ALT),
        &mut e,
        &mut camera,
    )?;
    let before = e.document().clone();
    let view = camera.viewport();
    let middle = |state| PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
        state,
        repeat: false,
    };
    input.physical(middle(ElementState::Pressed), &mut e, &mut camera)?;
    assert!(!input.images.active());
    input.cursor_moved([point[0] + 30., point[1] + 10.], &e, &mut camera)?;
    input.physical(middle(ElementState::Released), &mut e, &mut camera)?;
    assert_ne!(camera.viewport(), view);
    assert_eq!(e.document(), &before);
    let point = camera.world_to_screen(input.gizmo.handle(t, &camera, 4));
    input.cursor_moved(point, &e, &mut camera)?;
    input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
    assert!(input.images.active());
    input.cursor_moved([point[0] + 30., point[1] + 10.], &e, &mut camera)?;
    input.physical(button(ElementState::Released), &mut e, &mut camera)?;
    assert_eq!(
        e.document()
            .object(ObjectId::new(2)?)
            .ok_or("object")?
            .transform()
            .center(),
        t.center()
    );
    assert_eq!(e.undo_len(), 1);
    Ok(())
}

#[test]
fn already_held_middle_has_priority_over_later_alt_handle_press() -> Result {
    let mut e = fixture()?;
    let before = e.document().clone();
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input
        .images
        .selection
        .select(Some(ObjectId::new(2)?), false);
    let frame = input.images.frame(e.document()).ok_or("frame")?;
    let point = camera.world_to_screen(input.gizmo.handle(frame, &camera, 4));
    input.cursor_moved(point, &e, &mut camera)?;
    let middle = |state| PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
        state,
        repeat: false,
    };
    input.physical(middle(ElementState::Pressed), &mut e, &mut camera)?;
    input.physical(
        PhysicalEvent::Modifiers(Modifiers::ALT),
        &mut e,
        &mut camera,
    )?;
    input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
    let view = camera.viewport();
    input.cursor_moved([point[0] + 20., point[1] + 10.], &e, &mut camera)?;
    assert_ne!(camera.viewport(), view);
    assert_eq!(e.document(), &before);
    assert!(!input.images.active());
    input.physical(button(ElementState::Released), &mut e, &mut camera)?;
    input.physical(middle(ElementState::Released), &mut e, &mut camera)?;
    assert_eq!(e.undo_len(), 0);
    Ok(())
}

#[test]
fn middle_press_cancels_an_already_held_left_handle_gesture() -> Result {
    let mut e = fixture()?;
    let before = e.document().clone();
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input
        .images
        .selection
        .select(Some(ObjectId::new(2)?), false);
    let frame = input.images.frame(e.document()).ok_or("frame")?;
    let point = camera.world_to_screen(input.gizmo.handle(frame, &camera, 4));
    input.cursor_moved(point, &e, &mut camera)?;
    input.physical(
        PhysicalEvent::Modifiers(Modifiers::ALT),
        &mut e,
        &mut camera,
    )?;
    input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
    input.cursor_moved([point[0] + 30., point[1] + 10.], &e, &mut camera)?;
    assert!(input.images.active());
    let middle = |state| PhysicalEvent::Button {
        control: PhysicalControl::Pointer(PointerButton::Mouse(MouseButton::Middle)),
        state,
        repeat: false,
    };
    input.physical(middle(ElementState::Pressed), &mut e, &mut camera)?;
    assert!(!input.images.active());
    let view = camera.viewport();
    input.cursor_moved([point[0] + 60., point[1] + 20.], &e, &mut camera)?;
    assert_ne!(camera.viewport(), view);
    input.physical(button(ElementState::Released), &mut e, &mut camera)?;
    input.physical(middle(ElementState::Released), &mut e, &mut camera)?;
    assert_eq!(e.document(), &before);
    assert_eq!(e.undo_len(), 0);
    Ok(())
}

#[test]
fn save_request_during_preview_keeps_gesture_alive_and_excludes_preview() -> Result {
    let mut e = fixture()?;
    let before = e.document().clone();
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input.cursor_moved([640., 360.], &e, &mut camera)?;
    input.physical(button(ElementState::Pressed), &mut e, &mut camera)?;
    input.cursor_moved([670., 370.], &e, &mut camera)?;
    assert!(input.dispatch(
        ActionEvent {
            action: Action::Save,
            phase: ActionPhase::Invoke
        },
        &mut e,
        &mut camera
    )?);
    assert!(input.active());
    assert_eq!(e.document(), &before);
    input.physical(button(ElementState::Released), &mut e, &mut camera)?;
    assert_eq!(e.undo_len(), 1);
    Ok(())
}
