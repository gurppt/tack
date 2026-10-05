//! Generated local performance fixtures. No private media or external services.
use std::{ffi::OsString, path::PathBuf, sync::Arc, time::Instant};
use tack_assets::{AssetError, ProductAssets, image_metadata, source_fingerprint};
use tack_core::*;
use tack_storage::{self as storage, BlobInput, Payload, TackFile};
pub fn run(args: Vec<OsString>) -> Result<(), AssetError> {
    if args.len() != 2 {
        return Err("supply-scale NEW_DIRECTORY GENERATED_IMAGE_DIRECTORY".into());
    }
    let root = PathBuf::from(&args[0]);
    storage::create_private_directory(&root, false)?;
    let images = PathBuf::from(&args[1]);
    let mut pool = Vec::new();
    for i in 0..64 {
        let path = images.join(format!("{i:03}.png"));
        let size = image_metadata(&path)?.0;
        let source = Source::from_descriptor(
            storage::new_source_id()?,
            SourceLocation::Linked(LinkedPath::native(&path.canonicalize()?)?),
            1,
            Some(source_fingerprint(&path)?),
        )?;
        let asset = ImageAsset::new(storage::new_asset_id()?, source.id(), size)?;
        pool.push((source, asset));
    }
    let mut receipts = Vec::new();
    for (name, count) in [
        ("images-1k", 1000),
        ("images-5k", 5000),
        ("images-50k", 50000),
        ("mixed", 1000),
        ("sparse", 5000),
        ("shapes-10k", 10000),
        ("dense-highlod", 5000),
    ] {
        let start = Instant::now();
        let mut d = Document::new(storage::new_document_id()?, DocumentLimits::default());
        if name != "shapes-10k" {
            for (s, a) in &pool {
                d.apply(Command::AddSource(s.clone()))?;
                d.apply(Command::AddAsset(*a))?;
            }
        }
        for i in 0..count {
            let center = if name == "dense-highlod" {
                [0.; 2]
            } else if name == "sparse" {
                [
                    if i % 2 == 0 { 0. } else { 8e7 } + (i % 20) as f64 * 360.,
                    (i / 20) as f64 * 260.,
                ]
            } else {
                [(i % 100) as f64 * 360., (i / 100) as f64 * 260.]
            };
            let t = Transform::new(center, [320., 220.], 0., [false; 2])?;
            let id = storage::new_object_id()?;
            let object = if name == "shapes-10k" {
                DocumentObject::annotation(
                    id,
                    Annotation::new(
                        AnnotationKind::Rect,
                        AnnotationStyle::new(
                            Color([220, 190, 90, 255]),
                            None,
                            2.,
                            Opacity::OPAQUE,
                        )?,
                    ),
                    t,
                )?
            } else {
                DocumentObject::image(id, pool[i % 64].1.id(), t)
            };
            d.apply(Command::AddObject { object, index: i })?;
            if i % 2 == 0 && name != "shapes-10k" {
                d.apply(Command::SetImageFiltering {
                    object: id,
                    filtering: ImageFiltering::Nearest,
                })?;
            }
        }
        if name == "mixed" {
            let members = d.object_order()[..8].to_vec();
            d.apply(Command::AddGroup(Group::new(
                storage::new_group_id()?,
                members,
            )?))?;
            for i in 0..100 {
                let t = Transform::new(
                    [(i % 20) as f64 * 200., (i / 20) as f64 * 190.],
                    [180., 120.],
                    0.,
                    [false; 2],
                )?;
                let kind = if i % 2 == 0 {
                    AnnotationKind::Text(TextObject::new(
                        format!("Note {i} é猫"),
                        12.,
                        TextAlignment::Left,
                    )?)
                } else {
                    AnnotationKind::Arrow(LineObject::new([[0., 0.], [1., 1.]])?)
                };
                let object = DocumentObject::annotation(
                    storage::new_object_id()?,
                    Annotation::new(
                        kind,
                        AnnotationStyle::new(
                            Color([230, 170, 90, 255]),
                            None,
                            2.,
                            Opacity::OPAQUE,
                        )?,
                    ),
                    t,
                );
                d.apply(Command::AddObject {
                    object: object?,
                    index: d.object_order().len(),
                })?;
            }
            for i in 0..10 {
                let object = DocumentObject::frame(
                    storage::new_object_id()?,
                    format!("Area {i}"),
                    Transform::new([i as f64 * 1100., 300.], [1000., 1000.], 0., [false; 2])?,
                )?;
                d.apply(Command::AddObject { object, index: 0 })?;
            }
        }
        let path = root.join(format!("{name}.tack"));
        storage::save(&path, &d, vec![])?;
        // 64 safe source descriptors, shared by the generated image objects; preserve
        // per-asset previews but derive/refine source pixels only once per tier.
        if name != "shapes-10k" {
            let b = Arc::new(TackFile::open(&path)?);
            let mut a =
                ProductAssets::new(Arc::clone(&b), &path, root.join(format!("cache-{name}")))?;
            let mut index = 0;
            loop {
                a.poll();
                while index < pool.len() && a.request(pool[index].1.id()) {
                    index += 1;
                }
                if index == pool.len() && a.stats().pending == 0 {
                    break;
                }
                if start.elapsed().as_secs() > 120 {
                    return Err("fixture preparation deadline".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if a.stats().errors > 0 {
                return Err("generated fixture preview failure".into());
            }
            let blobs = a
                .prepared
                .values()
                .map(|p| {
                    BlobInput::overview(
                        p.asset,
                        p.revision,
                        p.size,
                        p.generator,
                        Payload::File(p.path.clone()),
                    )
                })
                .collect();
            storage::save(&path, &d, blobs)?;
        }
        receipts.push(serde_json::json!({"name":name,"objects":d.object_order().len(),"sources":d.sources().count(),"bytes":path.metadata()?.len(),"generate_ms":start.elapsed().as_secs_f64()*1000.}));
    }
    std::fs::write(
        root.join("fixtures.json"),
        serde_json::to_vec_pretty(&receipts)?,
    )?;
    Ok(())
}
