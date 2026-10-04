//! Generated, reproducible annotation metadata/render fixtures. No private media.
use serde_json::json;
use std::{ffi::OsString, path::PathBuf, time::Instant};
use tack_app::{
    annotation_scene::AnnotationScene, annotation_tool::AnnotationInput,
    image_interaction::ImageInteraction,
};
use tack_assets::AssetError;
use tack_core::*;
use tack_storage::{BlobInput, TackFile};
fn object(index: usize, kind: &str, points: &[[f64; 2]]) -> Result<DocumentObject, AssetError> {
    let (k, size, font) = match kind {
        "text" => (
            AnnotationKind::Text(TextObject::new(
                format!("Note {index} é猫\nreview"),
                4.,
                TextAlignment::Left,
            )?),
            [38., 18.],
            true,
        ),
        "scribble" => (
            AnnotationKind::Scribble(ScribbleObject::new(points.to_vec())?),
            [40., 20.],
            true,
        ),
        _ => (
            match index % 3 {
                0 => AnnotationKind::Rect,
                1 => AnnotationKind::Line(LineObject::new([[0., 0.], [1., 1.]])?),
                _ => AnnotationKind::Arrow(LineObject::new([[0., 1.], [1., 0.]])?),
            },
            [8., 3.],
            false,
        ),
    };
    let columns = if font { 32 } else { 100 };
    let pitch = if font { [40., 20.] } else { [10., 5.] };
    let t = Transform::new(
        [
            (index % columns) as f64 * pitch[0],
            (index / columns) as f64 * pitch[1],
        ],
        size,
        0.,
        [false; 2],
    )?;
    let style = AnnotationStyle::new(Color([255, 198, 82, 255]), None, 0.25, Opacity::OPAQUE)?;
    Ok(DocumentObject::annotation(
        ObjectId::new(index as u128 + 1)?,
        Annotation::new(k, style),
        t,
    )?)
}
pub fn run(args: Vec<OsString>) -> Result<(), AssetError> {
    if args.is_empty() || args.len() > 2 {
        return Err("annotation-scale NEW_DIRECTORY [PREPARED_BOARD] (benchmark only)".into());
    }
    let root = PathBuf::from(&args[0]);
    tack_storage::create_private_directory(&root, false)?;
    let raw: Vec<_> = (0..1000)
        .map(|i| [i as f64 / 999., 0.5 + (i as f64 * 0.035).sin() * 0.4])
        .collect();
    let mut simplify = Vec::new();
    let mut retained = Vec::new();
    for _ in 0..64 {
        let start = Instant::now();
        let s = simplify_stroke(&raw, 0.002)?;
        simplify.push(start.elapsed().as_secs_f64() * 1000.);
        retained = s;
    }
    let short = [[0., 0.], [0.3, 0.7], [0.7, 0.2], [1., 1.]];
    let mut runs = Vec::new();
    for (kind, count, points) in [
        ("shapes", 1000, &short[..]),
        ("shapes", 5000, &short[..]),
        ("shapes", 10000, &short[..]),
        ("text", 100, &short[..]),
        ("text", 1000, &short[..]),
        ("text", 5000, &short[..]),
        ("scribble", 1, &short[..]),
        ("scribble", 100, &retained[..]),
        ("scribble", 2, &raw[..]),
    ] {
        let mut d = Document::new(DocumentId::new(1)?, DocumentLimits::default());
        let start = Instant::now();
        for i in 0..count {
            d.apply(Command::AddObject {
                object: object(i, kind, points)?,
                index: i,
            })?;
        }
        let create_ms = start.elapsed().as_secs_f64() * 1000.;
        let p = root.join(format!("{kind}-{count}.tack"));
        let start = Instant::now();
        tack_storage::save(&p, &d, vec![])?;
        let save_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let b = TackFile::open(&p)?;
        let open_ms = start.elapsed().as_secs_f64() * 1000.;
        if d != b.document {
            return Err("annotation roundtrip invariant".into());
        }
        let mut camera = Camera::new([1280, 720]);
        camera.set_view([600., 300.], 1.)?;
        let mut query = Vec::new();
        let mut build = Vec::new();
        let mut scene = AnnotationScene::default();
        for _ in 0..64 {
            let start = Instant::now();
            std::hint::black_box(
                d.object_order()
                    .iter()
                    .filter_map(|id| d.object(*id))
                    .filter(|o| {
                        matches!(o.kind(), ObjectKind::Annotation(_))
                            && o.bounds().intersects(camera.viewport())
                    })
                    .count(),
            );
            query.push(start.elapsed().as_secs_f64() * 1000.);
            scene.build(
                &d,
                &ImageInteraction::default(),
                &AnnotationInput::default(),
                &camera,
                &[],
            );
            build.push(scene.build_ms);
        }
        runs.push(json!({"kind":kind,"count":count,"create_ms":create_ms,"save_ms":save_ms,"open_ms":open_ms,"query_ms":query,"build_ms":build,"primitives":scene.primitives.len(),"glyphs":scene.glyphs,"omitted":scene.omitted,"file_bytes":std::fs::metadata(p)?.len(),"invariant":true,"points_per_stroke":if kind=="scribble" {points.len()} else {0}}));
    }
    if let Some(input) = args.get(1) {
        let b = TackFile::open(PathBuf::from(input))?;
        let mut d = b.document.clone();
        let n = d.object_order().len();
        for i in 0..100 {
            let mut o = object(i, "shapes", &short)?;
            let id = ObjectId::new(u128::MAX - i as u128)?;
            let a = match o.kind() {
                ObjectKind::Annotation(a) => a.as_ref().clone(),
                _ => return Err("fixture kind".into()),
            };
            o = DocumentObject::annotation(id, a, o.transform())?;
            d.apply(Command::AddObject {
                object: o,
                index: n + i,
            })?;
        }
        d.apply(Command::AddObject {
            object: DocumentObject::frame(
                ObjectId::new(u128::MAX - 101)?,
                "Mixed annotations".into(),
                Transform::new([300., 200.], [800., 600.], 0., [false; 2])?,
            )?,
            index: n + 100,
        })?;
        let mut blobs = Vec::new();
        for (id, e) in &b.originals {
            blobs.push(BlobInput::original(*id, e.revision, b.payload(e.range)));
        }
        for (id, e) in &b.overviews {
            blobs.push(BlobInput::overview(
                *id,
                e.revision,
                [e.width, e.height],
                e.generator,
                b.payload(e.range),
            ));
        }
        tack_storage::save(root.join("mixed.tack"), &d, blobs)?;
    }
    let sizes = json!({"document":std::mem::size_of::<Document>(),"document_object":std::mem::size_of::<DocumentObject>(),"command":std::mem::size_of::<Command>(),"image_input":std::mem::size_of::<tack_app::image_input::ImageInput>(),"annotation":std::mem::size_of::<Annotation>(),"primitive":std::mem::size_of::<tack_render::AnnotationPrimitive>()});
    crate::report_output::write_new(
        &root.join("metadata.json"),
        &serde_json::to_vec_pretty(
            &json!({"runs":runs,"simplify_ms":simplify,"raw_points":1000,"retained_points":retained.len(),"sizes_bytes":sizes,"allocation_counts":"not instrumented"}),
        )?,
    )?;
    Ok(())
}
