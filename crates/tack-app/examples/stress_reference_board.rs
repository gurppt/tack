//! Cheap owned object-count fixtures; source counts are explicit, never invented.
use std::{fs, path::PathBuf};
use tack_assets::AssetError;
use tack_core::*;
fn main() -> Result<(), AssetError> {
    let root = PathBuf::from(std::env::args_os().nth(1).ok_or("new output directory")?);
    fs::create_dir(&root)?;
    fs::write(
        root.join("OWNER_STRESS.txt"),
        b"P0 D owned reference-count fixtures\n",
    )?;
    let source = std::env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("existing owned tiny image")?
        .canonicalize()?;
    let fingerprint = tack_assets::source_fingerprint(&source)?;
    let plan = tack_assets::image_import_plan(&source, fingerprint.size)?;
    if fingerprint.size > 1024 * 1024 {
        return Err("small stress source required".into());
    }
    for count in [1000, 4000, 10000] {
        let mut document =
            Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
        let source_id = tack_storage::new_source_id()?;
        let asset = tack_storage::new_asset_id()?;
        document.apply(Command::AddSource(Source::from_descriptor(
            source_id,
            SourceLocation::Linked(LinkedPath::native(&source)?),
            1,
            Some(fingerprint),
        )?))?;
        document.apply(Command::AddAsset(ImageAsset::new(
            asset,
            source_id,
            plan.pixels,
        )?))?;
        let side = (count as f64).sqrt().ceil() as usize;
        for i in 0..count {
            let position = [
                (i % side) as f64 * 8. - side as f64 * 4.,
                (i / side) as f64 * 6. - side as f64 * 3.,
            ];
            document.apply(Command::AddObject {
                object: DocumentObject::image(
                    tack_storage::new_object_id()?,
                    asset,
                    Transform::new(position, [7., 5.], 0., [false; 2])?,
                ),
                index: i,
            })?;
        }
        let path = root.join(format!("objects-{count}.tack"));
        tack_storage::save(&path, &document, vec![])?;
        println!(
            "{}",
            serde_json::json!({"board":path,"objects":count,"unique_sources":1,"unique_assets":1,"source_pixels":plan.pixels,"scope":"dense reused-source renderer control, not a unique-source stress claim"})
        );
    }
    Ok(())
}
