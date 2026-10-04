use tack_core::*;
type R = Result<(), Box<dyn std::error::Error>>;
#[test]
fn bounded_annotation_metadata_and_atomic_mixed_history() -> R {
    let mut e = DocumentEditor::new(
        Document::new(DocumentId::new(1)?, DocumentLimits::default()),
        200,
    );
    let transform = Transform::new([10., 20.], [40., 50.], 0.3, [false; 2])?;
    let id = ObjectId::new(1)?;
    let text = TextObject::new("Une note\n猫 Ж 😀 café".into(), 24., TextAlignment::Left)?;
    let original = DocumentObject::annotation(
        id,
        Annotation::new(
            AnnotationKind::Text(text.clone()),
            AnnotationStyle::default(),
        ),
        transform,
    )?;
    e.execute(Command::AddObject {
        object: original.clone(),
        index: 0,
    })?;
    assert_eq!(e.document().annotation_count(), 1);
    let before = e.document().clone();
    let generation = e.generation();
    assert!(
        e.execute(Command::Batch(vec![
            Command::SetText {
                object: id,
                text: TextObject::new("changed".into(), 32., TextAlignment::Right)?
            },
            Command::SetTransform {
                object: ObjectId::new(2)?,
                transform
            }
        ]))
        .is_err()
    );
    assert_eq!(e.document(), &before);
    assert_eq!(e.generation(), generation);
    e.execute(Command::SetOpacity {
        object: id,
        opacity: Opacity::new(0.4)?,
    })?;
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.execute(Command::SetText {
        object: id,
        text: TextObject::new("αβγ\nsecond line".into(), 32., TextAlignment::Center)?,
    })?;
    let changed = e.document().clone();
    e.undo()?;
    assert_eq!(e.document(), &before);
    e.redo()?;
    assert_eq!(e.document(), &changed);
    e.execute(Command::RemoveObject(id))?;
    assert_eq!(e.document().annotation_count(), 0);
    e.undo()?;
    assert_eq!(e.document(), &changed);
    assert!(
        Command::AddObject {
            object: original,
            index: 0
        }
        .retained_bytes()
            > std::mem::size_of::<Command>() + text.value().len()
    );
    assert!(TextObject::new("x".repeat(MAX_TEXT_BYTES + 1), 24., TextAlignment::Left).is_err());
    assert!(TextObject::new("bad\0".into(), 24., TextAlignment::Left).is_err());
    assert!(TextObject::new("".into(), f64::NAN, TextAlignment::Left).is_err());
    assert!(AnnotationStyle::new(Color([1; 4]), None, f64::INFINITY, Opacity::OPAQUE).is_err());
    assert!(ScribbleObject::new(vec![[0., 0.]; MAX_STROKE_POINTS + 1]).is_err());
    assert!(ScribbleObject::new(vec![[0., 0.], [f64::NAN, 0.]]).is_err());
    assert!(LineObject::new([[0., 0.], [2., 1.]]).is_err());
    let b = Annotation::new(
        AnnotationKind::Arrow(LineObject::new([[0., 0.5], [1., 0.5]])?),
        AnnotationStyle::default(),
    )
    .bounds(transform)?;
    assert!(b.x < transform.bounds().x);
    Ok(())
}
#[test]
fn simplification_preserves_endpoints_and_deviation_with_hard_bounds() -> R {
    let points: Vec<_> = (0..1000)
        .map(|i| [i as f64, (i as f64 / 40.).sin() * 10.])
        .collect();
    let retained = simplify_stroke(&points, 0.5)?;
    assert_eq!(retained.first(), points.first());
    assert_eq!(retained.last(), points.last());
    assert!(retained.len() < 100);
    for p in &points {
        assert!(
            retained
                .windows(2)
                .any(|q| segment_distance(*p, q[0], q[1]) <= 0.500001)
        );
    }
    assert_eq!(simplify_stroke(&points, 0.5)?, retained);
    assert_eq!(
        simplify_stroke(&[[0., 0.], [0., 0.], [0., 0.]], 0.5)?.len(),
        2
    );
    assert!(simplify_stroke(&vec![[0., 0.]; MAX_STROKE_POINTS + 1], 1.).is_err());
    assert!(simplify_stroke(&[[0., 0.], [f64::INFINITY, 1.]], 1.).is_err());
    assert!(simplify_stroke(&points, f64::NAN).is_err());
    Ok(())
}
#[test]
fn oversized_delete_inverse_is_rejected_without_losing_history_or_document()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    let points: Vec<_> = (0..4096)
        .map(|i| [i as f64 / 4095., (i % 2) as f64])
        .collect();
    for i in 1..=520 {
        d.apply(Command::AddObject {
            object: DocumentObject::annotation(
                ObjectId::new(i)?,
                Annotation::new(
                    AnnotationKind::Scribble(ScribbleObject::new(points.clone())?),
                    AnnotationStyle::default(),
                ),
                Transform::new([0.; 2], [100.; 2], 0., [false; 2])?,
            )?,
            index: d.object_order().len(),
        })?;
    }
    let mut e = DocumentEditor::new(d, 200);
    e.execute(Command::SetTransform {
        object: ObjectId::new(1)?,
        transform: Transform::new([10.; 2], [100.; 2], 0., [false; 2])?,
    })?;
    e.undo()?;
    e.mark_saved();
    let before = e.document().clone();
    let generation = e.generation();
    let bytes = e.history_bytes();
    let undo = e.undo_len();
    let redo = e.redo_len();
    let commands = e
        .document()
        .object_order()
        .iter()
        .map(|id| Command::RemoveObject(*id))
        .collect();
    assert!(matches!(
        e.execute(Command::Batch(commands)),
        Err(CommandError::LimitReached("history inverse bytes"))
    ));
    assert_eq!(e.document(), &before);
    assert_eq!(e.generation(), generation);
    assert_eq!(e.history_bytes(), bytes);
    assert_eq!(e.undo_len(), undo);
    assert_eq!(e.redo_len(), redo);
    assert!(!e.is_dirty());
    assert!(e.redo()?);
    Ok(())
}
