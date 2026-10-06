use tack_app::{
    actions::*,
    image_input::ImageInput,
    image_interaction::{GestureKind, SelectionState},
    input::*,
    spatial_layout::{self, Layout},
    spatial_snap::*,
};
use tack_core::*;
type R = Result<(), Box<dyn std::error::Error + Send + Sync>>;
fn fixture() -> Result<DocumentEditor, Box<dyn std::error::Error + Send + Sync>> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    d.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [100, 100],
    )?))?;
    for (i, x) in [0., 150., 350.].into_iter().enumerate() {
        d.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(i as u128 + 1)?,
                AssetId::new(1)?,
                Transform::new([x, 0.], [100., 50.], 0., [false; 2])?,
            ),
            index: i,
        })?;
    }
    Ok(DocumentEditor::new(d, 200))
}
fn invoke(input: &mut ImageInput, e: &mut DocumentEditor, c: &mut Camera, action: Action) -> R {
    input.dispatch(
        ActionEvent {
            action,
            phase: ActionPhase::Invoke,
        },
        e,
        c,
    )?;
    Ok(())
}
#[test]
fn snap_threshold_ties_hysteresis_grid_disable_and_rotated_bounds() -> R {
    let mut e = fixture()?;
    let mut selection = SelectionState::default();
    selection.select(Some(ObjectId::new(1)?), false);
    let mut snap = SnapState {
        enabled: true,
        ..Default::default()
    };
    let b = WorldRect::new(5., -25., 100., 50.)?;
    assert_eq!(
        snap.resolve(e.document(), &selection, b, 1., 1., None)[0],
        -5.
    );
    assert_eq!(snap.guide_object(0), Some(ObjectId::new(2)?));
    // Same latch stays stable throughout small pointer noise.
    for x in [5.1, 4.9, 5.2, 4.8] {
        assert!(
            (snap.resolve(
                e.document(),
                &selection,
                WorldRect::new(x, -25., 100., 50.)?,
                1.,
                1.,
                None
            )[0] + x)
                .abs()
                < 1e-10
        );
    }
    for (zoom, dpi) in [(0.1, 1.), (2., 1.), (2., 2.), (0.5, 1.25)] {
        snap.clear();
        let distance = 5. * dpi / zoom;
        assert!(
            (snap.resolve(
                e.document(),
                &selection,
                WorldRect::new(100. - distance, -25., 20., 50.)?,
                zoom,
                dpi,
                Some([0, 3])
            )[0] - distance)
                .abs()
                < 1e-10
        );
    }
    snap.disabled = true;
    assert_eq!(
        snap.resolve(e.document(), &selection, b, 1., 1., None),
        [0.; 2]
    );
    assert!(snap.guides.iter().all(Option::is_none));
    snap = SnapState {
        grid: true,
        ..Default::default()
    };
    let b = WorldRect::new(3., 3., 20., 20.)?;
    assert_eq!(
        snap.resolve(e.document(), &SelectionState::default(), b, 1., 1., None),
        [-3., -3.]
    );
    assert_eq!(grid_spacing(1., 1.), 32.);
    assert_eq!(grid_spacing(0.5, 1.), 64.);
    assert_eq!(grid_spacing(1., 2.), 64.);
    // A rotated candidate uses its world AABB, with stable ID tie-breaking.
    for id in [2, 3] {
        e.execute(Command::SetTransform {
            object: ObjectId::new(id)?,
            transform: Transform::new(
                [150., 0.],
                [100., 50.],
                std::f64::consts::FRAC_PI_2,
                [false; 2],
            )?,
        })?;
    }
    snap = SnapState {
        enabled: true,
        ..Default::default()
    };
    assert_eq!(
        snap.resolve(
            e.document(),
            &selection,
            WorldRect::new(20., 200., 100., 20.)?,
            1.,
            1.,
            None
        ),
        [5., 0.]
    );
    assert_eq!(snap.guide_object(0), Some(ObjectId::new(2)?));
    e.execute(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(99)?,
            "Frame snap target".into(),
            Transform::new([750., 200.], [100., 50.], 0., [false; 2])?,
        )?,
        index: 3,
    })?;
    snap.clear();
    assert_eq!(
        snap.resolve(
            e.document(),
            &selection,
            WorldRect::new(595., 300., 100., 20.)?,
            1.,
            1.,
            None
        ),
        [5., 0.]
    );
    assert_eq!(snap.guide_object(0), Some(ObjectId::new(99)?));
    Ok(())
}
#[test]
fn layout_missing_sources_rotated_bounds_atomic_undo_and_group_units() -> R {
    let mut e = fixture()?;
    let initial = e.document().clone();
    let id = ObjectId::new(2)?;
    e.execute(Command::SetTransform {
        object: id,
        transform: Transform::new([150., 15.], [80., 40.], 0.4, [false; 2])?,
    })?;
    let rotated = e.document().clone();
    let order = rotated.object_order().to_vec();
    for layout in [
        Layout::Left,
        Layout::HorizontalCenter,
        Layout::Right,
        Layout::Top,
        Layout::VerticalCenter,
        Layout::Bottom,
        Layout::DistributeHorizontal,
        Layout::DistributeVertical,
        Layout::PackHorizontal,
        Layout::PackVertical,
    ] {
        let command = spatial_layout::arrange(e.document(), order.iter().copied(), layout)?;
        assert_eq!(
            command,
            spatial_layout::arrange(e.document(), order.iter().copied(), layout)?
        );
        let before = e.undo_len();
        e.execute(command)?;
        assert_eq!(e.undo_len(), before + 1);
        assert_eq!(e.document().object_order(), order.as_slice());
        e.undo()?;
        assert_eq!(e.document(), &rotated);
    }
    e.undo()?;
    assert_eq!(e.document(), &initial);
    e.execute(Command::AddGroup(Group::new(
        GroupId::new(1)?,
        vec![ObjectId::new(1)?, ObjectId::new(2)?],
    )?))?;
    let grouped = e.document().clone();
    let relative = grouped
        .object(ObjectId::new(2)?)
        .ok_or("object")?
        .transform()
        .center()[0]
        - grouped
            .object(ObjectId::new(1)?)
            .ok_or("object")?
            .transform()
            .center()[0];
    e.execute(spatial_layout::arrange(
        e.document(),
        order.into_iter(),
        Layout::Right,
    )?)?;
    assert_eq!(
        e.document()
            .object(ObjectId::new(2)?)
            .ok_or("object")?
            .transform()
            .center()[0]
            - e.document()
                .object(ObjectId::new(1)?)
                .ok_or("object")?
                .transform()
                .center()[0],
        relative
    );
    e.undo()?;
    assert_eq!(e.document(), &grouped);
    Ok(())
}
#[test]
fn frame_group_semantic_actions_focus_and_cancel_preserve_document() -> R {
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input
        .images
        .selection
        .select(Some(ObjectId::new(1)?), false);
    input.images.selection.select(Some(ObjectId::new(2)?), true);
    invoke(&mut input, &mut e, &mut camera, Action::GroupSelection)?;
    let group = e.document().groups().next().ok_or("group")?.clone();
    input
        .images
        .selection
        .select_object(e.document(), Some(group.members()[0]), false);
    assert_eq!(input.images.selection.len(), 2);
    let original = e.document().clone();
    input.images.begin(GestureKind::Move, [0., 0.], &e)?;
    input.images.update([100., 200.])?;
    input.images.commit(&mut e)?;
    e.undo()?;
    assert_eq!(e.document(), &original);
    invoke(&mut input, &mut e, &mut camera, Action::CreateFrame)?;
    let id = input.images.selection.ids().next().ok_or("frame")?;
    let with_frame = e.document().clone();
    input.images.begin(
        GestureKind::Resize {
            handle: 3,
            center: false,
        },
        [0., 0.],
        &e,
    )?;
    input.images.update([100., 0.])?;
    input.images.cancel();
    assert_eq!(e.document(), &with_frame);
    invoke(&mut input, &mut e, &mut camera, Action::FocusFrame)?;
    assert!(
        camera
            .viewport()
            .intersects(with_frame.object(id).ok_or("frame")?.transform().bounds())
    );
    invoke(&mut input, &mut e, &mut camera, Action::NextFrame)?;
    assert!(input.images.selection.contains(id));
    invoke(&mut input, &mut e, &mut camera, Action::PreviousFrame)?;
    assert!(input.images.selection.contains(id));
    // A frame transform does not mutate geometrically contained images.
    input.images.begin(GestureKind::Move, [0., 0.], &e)?;
    input.images.update([100., 200.])?;
    input.images.commit(&mut e)?;
    for id in original.object_order() {
        assert_eq!(e.document().object(*id), original.object(*id));
    }
    e.undo()?;
    assert_eq!(e.document(), &with_frame);
    Ok(())
}
#[test]
fn x_hold_recomputes_preview_without_losing_pointer_capture() -> R {
    use winit::{event::ElementState, keyboard::KeyCode};
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input.snap.enabled = true;
    input
        .images
        .selection
        .select(Some(ObjectId::new(1)?), false);
    input.images.begin(GestureKind::Move, [0., 0.], &e)?;
    input.cursor_moved(camera.world_to_screen([55., 0.]), &e, &mut camera)?;
    let snapped = input
        .images
        .preview_transform(e.document(), ObjectId::new(1)?)
        .ok_or("preview")?;
    assert_eq!(snapped.center()[0], 50.);
    let key = PhysicalControl::LogicalKey(
        tack_app::input::LogicalKey::from_legacy(KeyCode::KeyX).ok_or("logical key")?,
    );
    input.physical(
        PhysicalEvent::Button {
            control: key,
            state: ElementState::Pressed,
            repeat: false,
        },
        &mut e,
        &mut camera,
    )?;
    assert!(input.images.active());
    assert_eq!(
        input
            .images
            .preview_transform(e.document(), ObjectId::new(1)?)
            .ok_or("preview")?
            .center()[0],
        55.
    );
    input.physical(
        PhysicalEvent::Button {
            control: key,
            state: ElementState::Released,
            repeat: false,
        },
        &mut e,
        &mut camera,
    )?;
    assert!(input.images.active());
    assert_eq!(
        input
            .images
            .preview_transform(e.document(), ObjectId::new(1)?)
            .ok_or("preview")?
            .center()[0],
        50.
    );
    input.physical(PhysicalEvent::FocusLost, &mut e, &mut camera)?;
    assert!(!input.images.active());
    assert_eq!(e.undo_len(), 0);
    Ok(())
}

#[test]
fn interior_marquee_ignores_containing_frame_and_overlap_distribution_avoids_protrusion() -> R {
    let mut e = fixture()?;
    e.execute(Command::AddObject {
        object: DocumentObject::frame(
            ObjectId::new(99)?,
            "Zone".into(),
            Transform::new([0., 0.], [1000., 1000.], 0., [false; 2])?,
        )?,
        index: 0,
    })?;
    let mut selection = SelectionState::default();
    selection.marquee(e.document(), WorldRect::new(-60., -30., 120., 60.)?, false);
    assert!(selection.contains(ObjectId::new(1)?));
    assert!(!selection.contains(ObjectId::new(99)?));
    selection.marquee(
        e.document(),
        WorldRect::new(-600., -600., 1200., 1200.)?,
        false,
    );
    assert!(selection.contains(ObjectId::new(99)?));
    for (id, x, width) in [(1, 0., 100.), (2, 1., 100.), (3, 2., 1.)] {
        e.execute(Command::SetTransform {
            object: ObjectId::new(id)?,
            transform: Transform::new([x + width / 2., 0.], [width, 50.], 0., [false; 2])?,
        })?;
    }
    e.execute(spatial_layout::arrange(
        e.document(),
        [ObjectId::new(1)?, ObjectId::new(2)?, ObjectId::new(3)?].into_iter(),
        Layout::DistributeHorizontal,
    )?)?;
    let b: Vec<_> = (1..=3)
        .map(|id| {
            e.document()
                .object(ObjectId::new(id).map_err(|e| e.to_string())?)
                .map(|o| o.transform().bounds())
                .ok_or("object".to_owned())
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(b[0].x, 0.);
    assert_eq!(b[1].x, 100.);
    assert_eq!(b[2].x, 200.);
    assert_eq!(b[2].x + b[2].width, 201.);
    Ok(())
}
#[test]
fn native_name_ime_is_utf8_bounded_and_f2_reusable_after_modal_release() -> R {
    use winit::{
        event::{ElementState, Ime, WindowEvent},
        keyboard::KeyCode,
    };
    let mut e = fixture()?;
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    invoke(&mut input, &mut e, &mut camera, Action::CreateFrame)?;
    let id = input.images.selection.ids().next().ok_or("frame")?;
    let f2 = PhysicalControl::LogicalKey(
        tack_app::input::LogicalKey::from_legacy(KeyCode::F2).ok_or("logical key")?,
    );
    for name in ["Références 猫", "Ж 😀"] {
        input.physical(
            PhysicalEvent::Button {
                control: f2,
                state: ElementState::Pressed,
                repeat: false,
            },
            &mut e,
            &mut camera,
        )?;
        assert!(input.name_edit.is_some());
        input.physical(
            PhysicalEvent::Button {
                control: f2,
                state: ElementState::Released,
                repeat: false,
            },
            &mut e,
            &mut camera,
        )?;
        input.handle(
            &WindowEvent::Ime(Ime::Commit(name.into())),
            &mut e,
            &mut camera,
        )?;
        assert_eq!(input.name_edit.as_ref().ok_or("name")?.value, name);
        input.finish_name_edit(&mut e, true)?;
        assert_eq!(
            e.document().object(id).ok_or("frame")?.kind(),
            &ObjectKind::Frame(name.into())
        );
    }
    invoke(&mut input, &mut e, &mut camera, Action::RenameFrame)?;
    input.handle(
        &WindowEvent::Ime(Ime::Commit("é".repeat(128))),
        &mut e,
        &mut camera,
    )?;
    input.handle(
        &WindowEvent::Ime(Ime::Commit("x".into())),
        &mut e,
        &mut camera,
    )?;
    assert_eq!(input.name_edit.as_ref().ok_or("name")?.value.len(), 256);
    let before = e.document().clone();
    input.handle(&WindowEvent::Focused(false), &mut e, &mut camera)?;
    assert_eq!(e.document(), &before);
    assert_eq!(
        input
            .name_edit
            .as_ref()
            .ok_or("retained draft")?
            .value
            .len(),
        256
    );
    input.finish_name_edit(&mut e, false)?;
    assert!(input.name_edit.is_none());
    Ok(())
}
#[test]
fn native_resize_snap_uses_active_edge_and_cancel_is_exact() -> R {
    let mut e = fixture()?;
    let before = e.document().clone();
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input.snap.enabled = true;
    input
        .images
        .selection
        .select(Some(ObjectId::new(1)?), false);
    input.images.begin(
        GestureKind::Resize {
            handle: 3,
            center: false,
        },
        [50., 0.],
        &e,
    )?;
    input.cursor_moved(camera.world_to_screen([95., 0.]), &e, &mut camera)?;
    let t = input
        .images
        .preview_transform(e.document(), ObjectId::new(1)?)
        .ok_or("transform")?;
    assert!((t.bounds().x + t.bounds().width - 100.).abs() < 1e-9);
    assert_eq!(t.bounds().x, -50.);
    invoke(&mut input, &mut e, &mut camera, Action::CancelInteraction)?;
    assert_eq!(e.document(), &before);
    assert!(input.snap.guides.iter().all(Option::is_none));
    Ok(())
}

#[test]
fn uniform_corner_snap_bounds_final_displacement_including_extreme_aspect_ratio() -> R {
    for (size, target, expected) in [
        ([1000., 10.], 10., [500., 5.]),
        ([100., 50.], 27., [54., 27.]),
    ] {
        for center in [false, true] {
            let mut e = fixture()?;
            e.execute(Command::RemoveObject(ObjectId::new(2)?))?;
            e.execute(Command::RemoveObject(ObjectId::new(3)?))?;
            e.execute(Command::SetTransform {
                object: ObjectId::new(1)?,
                transform: Transform::new([0., 0.], size, 0., [false; 2])?,
            })?;
            e.execute(Command::AddObject {
                object: DocumentObject::frame(
                    ObjectId::new(99)?,
                    "Distant horizontal target".into(),
                    Transform::new([5000., target + 5.], [100., 10.], 0., [false; 2])?,
                )?,
                index: 1,
            })?;
            let initial = e.document().clone();
            let mut input = ImageInput::new()?;
            let mut camera = Camera::new([1280, 720]);
            input.snap.enabled = true;
            input
                .images
                .selection
                .select(Some(ObjectId::new(1)?), false);
            let corner = size.map(|s| s / 2.);
            input
                .images
                .begin(GestureKind::Resize { handle: 4, center }, corner, &e)?;
            input.cursor_moved(camera.world_to_screen(corner), &e, &mut camera)?;
            let b = input
                .images
                .preview_transform(e.document(), ObjectId::new(1)?)
                .ok_or("preview")?
                .bounds();
            assert!((b.x + b.width - expected[0]).abs() < 1e-9);
            assert!((b.y + b.height - expected[1]).abs() < 1e-9);
            assert!(
                ((b.x + b.width - corner[0]).powi(2) + (b.y + b.height - corner[1]).powi(2)).sqrt()
                    <= SNAP_PIXELS
            );
            invoke(&mut input, &mut e, &mut camera, Action::CancelInteraction)?;
            assert_eq!(e.document(), &initial);
        }
    }
    Ok(())
}

#[test]
fn crowded_frames_keep_overlay_and_glyph_bounds_and_guide_priority() -> R {
    let mut document = fixture()?.document().clone();
    for id in 1000..5096 {
        document.apply(Command::AddObject {
            object: DocumentObject::frame(
                ObjectId::new(id)?,
                "Références 猫".into(),
                Transform::new([0., 0.], [400., 400.], 0., [false; 2])?,
            )?,
            index: document.object_order().len(),
        })?;
    }
    let e = DocumentEditor::new(document, 200);
    let mut input = ImageInput::new()?;
    let mut camera = Camera::new([1280, 720]);
    input.snap.enabled = true;
    input
        .images
        .selection
        .select(Some(ObjectId::new(1)?), false);
    input.images.begin(GestureKind::Move, [0., 0.], &e)?;
    input.cursor_moved(camera.world_to_screen([55., 0.]), &e, &mut camera)?;
    assert!(input.snap.guides.iter().all(Option::is_some));
    input.build_overlay(&e, &camera);
    assert!(input.gizmo.quads.len() <= tack_render::MAX_OVERLAY_QUADS);
    let glyphs = input
        .gizmo
        .quads
        .iter()
        .filter(|q| q.bitmap.is_some())
        .count();
    assert!(glyphs > 0 && glyphs <= 512);
    assert!(
        input
            .gizmo
            .quads
            .iter()
            .rev()
            .take(2)
            .all(|q| q.color == input.gizmo.palette.guide)
    );
    Ok(())
}
