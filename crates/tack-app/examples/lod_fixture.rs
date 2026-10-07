//! Small deterministic zoom/crop/rotation/group fixture using caller-owned PNGs.
use std::path::PathBuf;
use tack_core::*;
fn main() -> Result<(), tack_assets::AssetError> {
    let root =
        PathBuf::from(std::env::args_os().nth(1).ok_or("fixture directory")?).canonicalize()?;
    let mut document = Document::new(tack_storage::new_document_id()?, DocumentLimits::default());
    let mut group = Vec::new();
    for i in 0..8 {
        let path = root.join(format!("{i}.png"));
        let source = Source::from_descriptor(
            tack_storage::new_source_id()?,
            SourceLocation::Linked(LinkedPath::native(&path)?),
            1,
            Some(tack_assets::source_fingerprint(&path)?),
        )?;
        let asset = ImageAsset::new(tack_storage::new_asset_id()?, source.id(), [2048, 2048])?;
        document.apply(Command::AddSource(source))?;
        document.apply(Command::AddAsset(asset))?;
        let id = tack_storage::new_object_id()?;
        let center = if i == 0 {
            [0., 0.]
        } else {
            [(i % 4) as f64 * 720., (i / 4) as f64 * 720.]
        };
        let transform = Transform::new(
            center,
            [600., 600.],
            if i == 2 { 0.3 } else { 0. },
            [false; 2],
        )?;
        document.apply(Command::AddObject {
            object: DocumentObject::image(id, asset.id(), transform),
            index: i,
        })?;
        if i % 2 == 1 {
            document.apply(Command::SetImageFiltering {
                object: id,
                filtering: ImageFiltering::Nearest,
            })?;
        }
        if i < 2 {
            group.push(id);
        }
    }
    document.apply(Command::AddGroup(Group::new(
        tack_storage::new_group_id()?,
        group,
    )?))?;
    tack_storage::save(root.join("zoom.tack"), &document, vec![])?;
    Ok(())
}
