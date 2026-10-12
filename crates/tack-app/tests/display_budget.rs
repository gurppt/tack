use tack_app::display_budget::{Images, tile_group_fits};
use tack_core::*;
use tack_render::{DrawProductImage, MAX_IMAGE_QUADS};
type R<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
fn draw(i: usize) -> R<DrawProductImage> {
    Ok(DrawProductImage {
        data: ImageRenderData {
            object_id: ObjectId::new(i as u128 + 1)?,
            asset_id: AssetId::new(1)?,
            transform: Transform::new([0.; 2], [16.; 2], 0.3, [true, false])?,
            crop: Crop::new(0.1, 0.2, 0.6, 0.5)?,
            opacity: Opacity::new(0.4)?,
            filtering: ImageFiltering::Nearest,
            grayscale: i.is_multiple_of(2),
        },
        key: None,
    })
}
#[test]
fn admission_keeps_selected_then_frontmost_in_original_order_and_counts_all() -> R {
    for n in [9999, 10000, 10001, 20000, 100000] {
        let mut draws = Vec::with_capacity(MAX_IMAGE_QUADS);
        let mut budget = Images::new(&mut draws);
        for i in 0..n {
            budget.push(draw(i)?, |id| id.value() <= 2);
        }
        let result = budget.finish();
        assert_eq!(result.visible, n);
        assert_eq!(result.omitted, n.saturating_sub(MAX_IMAGE_QUADS));
        assert_eq!(result.selected_omitted, 0);
        assert_eq!(draws.len(), n.min(MAX_IMAGE_QUADS));
        assert_eq!(draws[0].data.object_id.value(), 1);
        assert_eq!(draws[1].data.object_id.value(), 2);
        if n > MAX_IMAGE_QUADS {
            assert_eq!(
                draws[2].data.object_id.value(),
                (n - MAX_IMAGE_QUADS + 3) as u128
            );
        }
        assert!(
            draws
                .windows(2)
                .all(|w| w[0].data.object_id < w[1].data.object_id)
        );
        for d in &draws {
            assert_eq!(d.data, draw(d.data.object_id.value() as usize - 1)?.data);
        }
        assert_eq!(result.warning(false).is_some(), n > MAX_IMAGE_QUADS);
        let mut all = Images::new(&mut draws);
        for i in 0..n {
            all.push(draw(i)?, |_| true);
        }
        assert_eq!(
            all.finish().selected_omitted,
            n.saturating_sub(MAX_IMAGE_QUADS)
        );
        // Camera recovery: re-use the exact same storage with a sparse viewport.
        let mut sparse = Images::new(&mut draws);
        sparse.push(draw(0)?, |_| false);
        assert_eq!(sparse.finish().warning(false), None);
        assert_eq!(draws.len(), 1);
    }
    Ok(())
}
#[test]
fn tile_expansion_reserves_complete_fallbacks_and_checks_overflow() {
    assert!(tile_group_fits(9996, 3, 1));
    assert!(!tile_group_fits(9996, 4, 1));
    assert!(tile_group_fits(0, 64, 9936));
    assert!(!tile_group_fits(0, 65, 9936));
    assert!(!tile_group_fits(usize::MAX, 1, 0));
}
#[test]
fn limited_draws_preserve_interleaved_annotation_order_and_document() -> R {
    let mut doc = Document::new(DocumentId::new(21)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "missing.png",
    )?))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [16, 16],
    )?))?;
    for i in 0..10001 {
        doc.apply(Command::AddObject {
            object: DocumentObject::image_with_properties(
                draw(i)?.data.object_id,
                AssetId::new(1)?,
                draw(i)?.data.transform,
                draw(i)?.data.crop,
                draw(i)?.data.opacity,
                ImageFiltering::Nearest,
            ),
            index: i,
        })?;
    }
    doc.apply(Command::AddObject {
        object: DocumentObject::annotation(
            ObjectId::new(20000)?,
            Annotation::new(AnnotationKind::Rect, AnnotationStyle::default()),
            Transform::new([0.; 2], [32.; 2], 0., [false; 2])?,
        )?,
        index: 5000,
    })?;
    let before = doc.clone();
    let mut draws = Vec::new();
    let mut admission = Images::new(&mut draws);
    for id in doc.object_order() {
        if let Some(data) = doc.object_render_data(*id) {
            admission.push(DrawProductImage { data, key: None }, |_| false);
        }
    }
    assert_eq!(admission.finish().omitted, 1);
    let mut scene = tack_app::annotation_scene::AnnotationScene::default();
    scene.build(
        &doc,
        &tack_app::image_interaction::ImageInteraction::default(),
        &tack_app::annotation_tool::AnnotationInput::default(),
        &Camera::new([800, 600]),
        &draws,
    );
    assert_eq!(doc, before);
    let a = scene
        .order
        .iter()
        .position(|d| matches!(d, tack_render::CanvasDraw::Annotations { .. }))
        .ok_or("annotation missing")?;
    assert_eq!(scene.order[a - 1], tack_render::CanvasDraw::Image(4998));
    assert_eq!(scene.order[a + 1], tack_render::CanvasDraw::Image(4999));
    assert_eq!(
        scene
            .order
            .iter()
            .filter(|d| matches!(d, tack_render::CanvasDraw::Image(_)))
            .count(),
        MAX_IMAGE_QUADS
    );
    Ok(())
}
