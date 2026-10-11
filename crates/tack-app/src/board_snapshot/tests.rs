use super::*;
use tack_core::*;
#[test]
fn dimensions_reject_huge_nonfinite_and_pixel_overflow_before_allocation() {
    let region = |w, h| WorldRect {
        x: 0.,
        y: 0.,
        width: w,
        height: h,
    };
    assert_eq!(dimensions(region(100.5, 80.25)).ok(), Some([101, 81]));
    assert!(dimensions(region(50000., 50000.)).is_err());
    assert!(dimensions(region(4096., 4096.)).is_err());
    assert!(dimensions(region(0., 1.)).is_err());
    assert!(dimensions(region(f64::NAN, 1.)).is_err());
    assert!(dimensions(region(4096., 2048.)).is_ok());
}
#[test]
fn window_move_hit_policy_includes_locked_images_frame_tabs_and_notes() -> Result<(), AssetError> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::embedded(SourceId::new(1)?)))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(1)?,
        SourceId::new(1)?,
        [32, 32],
    )?))?;
    doc.apply(Command::AddObject {
        object: DocumentObject::image(
            ObjectId::new(1)?,
            AssetId::new(1)?,
            Transform::new([0., 0.], [64., 64.], 0., [false, false])?,
        ),
        index: 0,
    })?;
    doc.apply(Command::SetLocked {
        object: ObjectId::new(1)?,
        locked: true,
    })?;
    let frame = Transform::new([150., 0.], [64., 64.], 0., [false, false])?;
    doc.apply(Command::AddObject {
        object: DocumentObject::frame(ObjectId::new(2)?, "Frame".into(), frame)?,
        index: 1,
    })?;
    let input = crate::image_input::ImageInput::new()?;
    let camera = Camera::new([800, 600]);
    assert!(!input.empty_canvas_at(&doc, &camera, [400., 300.]));
    let tab = crate::frame_ui::Label::new(frame, &camera, 1, "Frame").rect;
    assert!(!input.empty_canvas_at(
        &doc,
        &camera,
        [(tab[0] + tab[2]) / 2., (tab[1] + tab[3]) / 2.]
    ));
    assert!(input.empty_canvas_at(&doc, &camera, [50., 100.]));
    Ok(())
}
#[test]
#[ignore = "explicit offscreen snapshot GPU/PNG export"]
fn snapshot_uses_board_pixels_and_keeps_destination_on_cancel() -> Result<(), AssetError> {
    let path = std::env::temp_dir().join(format!(
        "tack-snapshot-test-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    std::fs::create_dir(&path)?;
    let result = (|| -> Result<(), AssetError> {
        let source = path.join("source.png");
        let original = Decoded {
            width: 16,
            height: 16,
            rgba: [230, 20, 40, 255].repeat(256),
        };
        tack_assets::snapshot::write_png(&original, &source)?;
        let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
        doc.apply(Command::AddSource(Source::linked(
            SourceId::new(1)?,
            &source,
        )?))?;
        doc.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(1)?,
            SourceId::new(1)?,
            [16, 16],
        )?))?;
        doc.apply(Command::AddObject {
            object: DocumentObject::image(
                ObjectId::new(1)?,
                AssetId::new(1)?,
                Transform::new([0., 0.], [32., 32.], 0., [false, false])?,
            ),
            index: 0,
        })?;
        let boardpath = path.join("board.tack");
        tack_storage::save(&boardpath, &doc, vec![])?;
        let board = TackFile::open(&boardpath)?;
        let request = Request::new(
            &doc,
            &SelectionState::default(),
            &boardpath,
            &board,
            &Originals::new(),
            &Preferences::defaults()?,
        )?;
        let pixels = request.render(&AtomicBool::new(false))?;
        assert_eq!([pixels.width, pixels.height], [80, 80]);
        let center = ((40 * 80 + 40) * 4) as usize;
        assert_eq!(&pixels.rgba[center..center + 4], &[230, 20, 40, 255]);
        let target = path.join("snapshot.png");
        request.export_to(&target, &AtomicBool::new(false))?;
        let bytes = std::fs::read(&target)?;
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert!(
            request
                .export_to(&path.join("snapshot"), &AtomicBool::new(false))
                .is_err()
        );
        assert!(request.export_to(&target, &AtomicBool::new(true)).is_err());
        assert_eq!(std::fs::read(&target)?, bytes);
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(path);
    result
}

#[test]
#[ignore = "explicit multi-source offscreen scene GPU export"]
fn snapshot_renders_nine_sources_embedded_text_shapes_and_frame_labels() -> Result<(), AssetError> {
    let root = std::env::temp_dir().join(format!(
        "tack-snapshot-scene-{:032x}",
        tack_storage::new_document_id()?.value()
    ));
    std::fs::create_dir(&root)?;
    let result = (|| -> Result<(), AssetError> {
        let source = root.join("original.png");
        tack_assets::snapshot::write_png(
            &Decoded {
                width: 16,
                height: 16,
                rgba: [230, 20, 40, 255].repeat(256),
            },
            &source,
        )?;
        let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
        let mut blobs = Vec::new();
        for n in 1..=9 {
            let sid = SourceId::new(n)?;
            doc.apply(Command::AddSource(if n == 1 {
                Source::linked(sid, &source)?
            } else {
                blobs.push(tack_storage::BlobInput::original(
                    sid,
                    1,
                    Payload::File(source.clone()),
                ));
                Source::embedded(sid)
            }))?;
            doc.apply(Command::AddAsset(ImageAsset::new(
                AssetId::new(n)?,
                sid,
                [16, 16],
            )?))?;
            doc.apply(Command::AddObject {
                object: DocumentObject::image(
                    ObjectId::new(n)?,
                    AssetId::new(n)?,
                    Transform::new([(n - 1) as f64 * 16., 0.], [12., 12.], 0., [false, false])?,
                ),
                index: doc.object_order().len(),
            })?;
        }
        doc.apply(Command::AddObject {
            object: DocumentObject::frame(
                ObjectId::new(10)?,
                "Frame".into(),
                Transform::new([64., 0.], [180., 100.], 0., [false, false])?,
            )?,
            index: doc.object_order().len(),
        })?;
        let color = Color([0, 0, 255, 255]);
        doc.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(11)?,
                Annotation::new(
                    AnnotationKind::Rect,
                    AnnotationStyle::new(color, Some(color), 1., Opacity::OPAQUE)?,
                ),
                Transform::new([100., 30.], [20., 16.], 0., [false, false])?,
            )?,
            index: doc.object_order().len(),
        })?;
        doc.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(12)?,
                Annotation::new(
                    AnnotationKind::Text(TextObject::new("Note".into(), 16., TextAlignment::Left)?),
                    AnnotationStyle::default(),
                ),
                Transform::new([20., -28.], [70., 22.], 0., [false, false])?,
            )?,
            index: doc.object_order().len(),
        })?;
        let path = root.join("scene.tack");
        tack_storage::save(&path, &doc, blobs)?;
        let board = TackFile::open(&path)?;
        let profile = Preferences::defaults()?;
        let request = Request::new(
            &doc,
            &SelectionState::default(),
            &path,
            &board,
            &Originals::new(),
            &profile,
        )?;
        let pixels = request.render(&AtomicBool::new(false))?;
        let pixel = |world: [f64; 2]| {
            let x = (world[0] - request.region.x).floor() as usize;
            let y = (world[1] - request.region.y).floor() as usize;
            &pixels.rgba[(y * pixels.width as usize + x) * 4..][..4]
        };
        assert_eq!(pixel([0., 0.]), &[230, 20, 40, 255]);
        assert_eq!(pixel([128., 0.]), &[230, 20, 40, 255]); // Ninth distinct GPU product.
        assert_eq!(pixel([100., 30.]), &[0, 0, 255, 255]);
        assert_ne!(pixel([-26., 0.]), pixel([-35., 0.])); // Frame outline.
        let text_pixels = (-37..-17)
            .flat_map(|y| (-10..50).map(move |x| [f64::from(x), f64::from(y)]))
            .filter(|p| {
                let c = pixel(*p);
                c[0] > 200 && c[1] > 150 && c[2] < 200
            })
            .count();
        assert!(text_pixels > 20, "authored note glyphs must appear");
        let mut selection = SelectionState::default();
        selection.select(Some(ObjectId::new(1)?), false);
        let selected = Request::new(&doc, &selection, &path, &board, &Originals::new(), &profile)?;
        assert_eq!(selected.size, [60, 60]);
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&root);
    result
}
