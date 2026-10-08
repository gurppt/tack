//! Deterministic metadata stress fixture; one existing original, no generated pixel corpus.
use std::path::PathBuf;
use tack_core::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 5 {
        return Err("fixture UNUSED.tack IMAGE IMAGES SHAPES NOTES".into());
    }
    let output = PathBuf::from(&args[0]);
    let counts: Vec<usize> = args[2..]
        .iter()
        .map(|a| {
            a.to_str()
                .ok_or("count text")?
                .parse::<usize>()
                .map_err(|_| "count integer")
        })
        .collect::<Result<_, _>>()?;
    if counts[0] > 1000 || counts[1] > 10000 || counts[2] > 100 {
        return Err("fixture caps 1000 images,10000 shapes,100 notes".into());
    }
    let mut doc = Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
    if counts[0] > 0 {
        let source = SourceId::new(1)?;
        doc.apply(Command::AddSource(Source::linked(
            source,
            std::path::absolute(&args[1])?,
        )?))?;
        doc.apply(Command::AddAsset(ImageAsset::new(
            AssetId::new(1)?,
            source,
            [400, 200],
        )?))?;
    }
    for index in 0..counts.iter().sum() {
        let id = ObjectId::new(index as u128 + 1)?;
        let position = [(index % 100) as f64 * 120., (index / 100) as f64 * 80.];
        let transform = Transform::new(position, [100., 60.], 0., [false; 2])?;
        let object = if index < counts[0] {
            DocumentObject::image(id, AssetId::new(1)?, transform)
        } else {
            let kind = if index < counts[0] + counts[1] {
                AnnotationKind::Rect
            } else {
                AnnotationKind::Text(TextObject::new(
                    format!("Note {index} | café 猫"),
                    12.,
                    TextAlignment::Left,
                )?)
            };
            DocumentObject::annotation(
                id,
                Annotation::new(kind, AnnotationStyle::default()),
                transform,
            )?
        };
        doc.apply(Command::AddObject { object, index })?;
    }
    let owner = tack_storage::BoardLease::acquire_new(&output)?;
    owner.save(&doc, Vec::new())?;
    println!(
        "{}",
        serde_json::json!({"board":format!("{:032x}",doc.id().value()),"images":counts[0],"shapes":counts[1],"notes":counts[2]})
    );
    Ok(())
}
