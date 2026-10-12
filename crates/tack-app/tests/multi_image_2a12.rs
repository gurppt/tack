//! Authorized compact UI correction, independent of the Turbo audit measurements.
use tack_app::{
    actions::{Action, ActionEvent, ActionPhase},
    context_menu::{self, Command as MenuCommand, Context, Group},
    image_input::ImageInput,
};
use tack_core::*;
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn fixture(n: usize) -> R<(DocumentEditor, ImageInput, Camera)> {
    let mut d = Document::new(DocumentId::new(1200)?, DocumentLimits::default());
    d.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    for i in 1..=n {
        let id = ObjectId::new(i as u128)?;
        let asset = AssetId::new(i as u128)?;
        d.apply(Command::AddAsset(ImageAsset::new(
            asset,
            SourceId::new(1)?,
            [200 + i as u32 * 2, 100 + i as u32 * 2],
        )?))?;
        d.apply(Command::AddObject {
            object: DocumentObject::image_with_properties(
                id,
                asset,
                Transform::new([i as f64 * 30., 15.], [40., 30.], 0.3, [true, false])?,
                Crop::new(0.1, 0.2, 0.5, 0.5)?,
                Opacity::OPAQUE,
                ImageFiltering::Nearest,
            ),
            index: i - 1,
        })?;
    }
    let e = DocumentEditor::new(d, 128);
    let mut input = ImageInput::new()?;
    input.images.selection.select_all(e.document());
    Ok((e, input, Camera::new([800, 600])))
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
fn size_and_crop_per_image_are_atomic_and_keep_heterogeneous_geometry() -> R {
    for n in [1, 2, 25] {
        let (mut e, mut i, mut c) = fixture(n)?;
        let before = e.document().clone();
        invoke(&mut i, &mut e, &mut c, Action::ActualSize)?;
        assert_eq!(e.undo_len(), 1);
        for id in i.images.selection.ids() {
            let a = e.document().object_render_data(id).ok_or("image")?;
            let old = before.object_render_data(id).ok_or("old")?;
            let pixels = before.asset(old.asset_id).ok_or("asset")?.pixel_size();
            assert_eq!(a.transform.size(), pixels.map(|v| v as f64 * 0.5));
            assert_eq!(a.transform.center(), old.transform.center());
            assert_eq!(a.transform.rotation(), old.transform.rotation());
            assert_eq!(a.transform.flips(), old.transform.flips());
            assert_eq!(a.crop, old.crop);
        }
        e.undo()?;
        assert_eq!(&before, e.document());
        e.redo()?;
        e.undo()?;
        invoke(&mut i, &mut e, &mut c, Action::ResetCrop)?;
        assert_eq!(e.undo_len(), 1);
        for id in i.images.selection.ids() {
            let a = e.document().object_render_data(id).ok_or("image")?;
            assert_eq!(a.crop, Crop::FULL);
            assert_eq!(a.transform, before.object(id).ok_or("old")?.transform());
        }
        e.undo()?;
        assert_eq!(&before, e.document());
    }
    Ok(())
}
#[test]
fn locked_color_cannot_prevent_editable_gray_images_returning_to_color() -> R {
    let (base, _, _) = fixture(2)?;
    let mut d = base.document().clone();
    let a = ObjectId::new(1)?;
    let b = ObjectId::new(2)?;
    d.apply(Command::SetGrayscale {
        object: a,
        grayscale: true,
    })?;
    d.apply(Command::SetLocked {
        object: b,
        locked: true,
    })?;
    let mut e = DocumentEditor::new(d, 128);
    let mut i = ImageInput::new()?;
    for id in [a, b] {
        i.images.selection.select(Some(id), true);
    }
    let mut c = Camera::new([800, 600]);
    let before = e.document().clone();
    invoke(&mut i, &mut e, &mut c, Action::ToggleGrayscale)?;
    assert!(!e.document().object_render_data(a).ok_or("image")?.grayscale);
    assert_eq!(e.document().object(b), before.object(b));
    assert_eq!(e.undo_len(), 1);
    assert!(i.status.contains("1 locked images unchanged"));
    e.undo()?;
    assert_eq!(e.document(), &before);
    for action in [Action::ActualSize, Action::ResetCrop] {
        invoke(&mut i, &mut e, &mut c, action)?;
        assert_eq!(e.document().object(b), before.object(b));
        assert_eq!(e.undo_len(), 1);
        e.undo()?;
    }
    i.images.selection.clear();
    i.images.selection.select(Some(b), true);
    let generation = e.generation();
    invoke(&mut i, &mut e, &mut c, Action::ToggleGrayscale)?;
    assert_eq!(e.generation(), generation);
    assert_eq!(e.undo_len(), 0);
    assert!(i.status.contains("Unlock"));
    Ok(())
}
#[test]
fn grayscale_normalizes_only_eligible_images_and_menu_reflects_the_target() -> R {
    for gray in [[false, false], [true, true], [true, false]] {
        let (base, _, _) = fixture(2)?;
        let mut d = base.document().clone();
        for (index, grayscale) in gray.into_iter().enumerate() {
            d.apply(Command::SetGrayscale {
                object: ObjectId::new(index as u128 + 1)?,
                grayscale,
            })?;
        }
        let mut e = DocumentEditor::new(d, 128);
        let mut i = ImageInput::new()?;
        i.images.selection.select_all(e.document());
        let mut c = Camera::new([800, 600]);
        let before = e.document().clone();
        let ctx = Context::selection(&e, &i.images.selection, false, false);
        let map = tack_app::image_input::product_keymap()?;
        let item = context_menu::items(ctx, Some(Group::Images), &map)
            .into_iter()
            .find(|x| x.command == MenuCommand::Action(Action::ToggleGrayscale))
            .ok_or("grayscale entry")?;
        assert_eq!(item.checked, gray.into_iter().all(|x| x));
        invoke(&mut i, &mut e, &mut c, Action::ToggleGrayscale)?;
        for id in i.images.selection.ids() {
            assert_eq!(
                e.document()
                    .object_render_data(id)
                    .ok_or("image")?
                    .grayscale,
                !gray.into_iter().all(|x| x)
            );
        }
        assert_eq!(e.undo_len(), 1);
        e.undo()?;
        assert_eq!(e.document(), &before);
    }
    Ok(())
}
#[test]
fn presets_flips_and_sampling_cycle_have_one_undo_and_refuse_locked_batches() -> R {
    use tack_app::selection_commands::Alpha;
    for n in [1, 2, 25] {
        for action in [
            Action::FlipHorizontal,
            Action::FlipVertical,
            Action::Opacity(Alpha::Half),
            Action::Filtering(ImageFiltering::Smooth),
            Action::CycleFiltering,
        ] {
            let (mut e, mut i, mut c) = fixture(n)?;
            let before = e.document().clone();
            invoke(&mut i, &mut e, &mut c, action)?;
            for id in i.images.selection.ids() {
                let a = e.document().object_render_data(id).ok_or("image")?;
                let old = before.object_render_data(id).ok_or("old")?;
                match action {
                    Action::FlipHorizontal => assert_eq!(a.transform.flips(), [false, false]),
                    Action::FlipVertical => assert_eq!(a.transform.flips(), [true, true]),
                    Action::Opacity(_) => assert_eq!(a.opacity.value(), 0.5),
                    Action::Filtering(_) => assert_eq!(a.filtering, ImageFiltering::Smooth),
                    Action::CycleFiltering => assert_eq!(a.filtering, ImageFiltering::Default),
                    _ => return Err("unexpected action".into()),
                }
                assert_eq!(a.transform.center(), old.transform.center());
                assert_eq!(a.crop, old.crop);
            }
            assert_eq!(e.undo_len(), 1);
            e.undo()?;
            assert_eq!(e.document(), &before);
            let mut locked = before.clone();
            locked.apply(Command::SetLocked {
                object: ObjectId::new(1)?,
                locked: true,
            })?;
            let mut e = DocumentEditor::new(locked.clone(), 128);
            invoke(&mut i, &mut e, &mut c, action)?;
            assert_eq!(e.document(), &locked);
            assert_eq!(e.undo_len(), 0);
        }
    }
    Ok(())
}
#[test]
fn multiple_menu_image_child_is_one_level_deep_and_mixed_objects_disable_image_actions() -> R {
    let (mut e, mut i, mut c) = fixture(2)?;
    let map = tack_app::image_input::product_keymap()?;
    let ctx = Context::selection(&e, &i.images.selection, false, false);
    assert!(
        context_menu::items(ctx, None, &map)
            .iter()
            .any(|x| x.command == MenuCommand::Submenu(Group::Images))
    );
    let items = context_menu::items(ctx, Some(Group::Images), &map);
    assert!(
        items
            .iter()
            .all(|x| !matches!(x.command, MenuCommand::Submenu(_)))
    );
    for a in [
        Action::ActualSize,
        Action::ToggleGrayscale,
        Action::ResetCrop,
        Action::Filtering(ImageFiltering::Nearest),
    ] {
        assert!(
            items
                .iter()
                .any(|x| x.command == MenuCommand::Action(a) && x.enabled)
        );
    }
    let frame = ObjectId::new(100)?;
    e.execute(Command::AddObject {
        object: DocumentObject::frame(
            frame,
            "frame".into(),
            Transform::new([100., 100.], [50.; 2], 0., [false; 2])?,
        )?,
        index: 2,
    })?;
    i.images.selection.select(Some(frame), true);
    let before = e.document().clone();
    invoke(&mut i, &mut e, &mut c, Action::ToggleGrayscale)?;
    assert_eq!(e.document(), &before);
    let ctx = Context::selection(&e, &i.images.selection, false, false);
    assert!(
        !context_menu::items(ctx, None, &map)
            .iter()
            .any(|x| x.command == MenuCommand::Submenu(Group::Images))
    );
    Ok(())
}
