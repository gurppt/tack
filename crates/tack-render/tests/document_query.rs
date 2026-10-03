use tack_core::*;

/// Render-side consumption of logical resident metadata, with no source resolver
/// or GPU needed. This does not claim the prototype draws rotated/cropped objects.
#[test]
fn renderer_consumer_sees_stable_assets_and_order_without_source_locations()
-> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Document::new(DocumentId::new(1)?, DocumentLimits::default());
    doc.apply(Command::AddSource(Source::linked(
        SourceId::new(1)?,
        "/missing/large/source.jpg",
    )?))?;
    doc.apply(Command::AddAsset(ImageAsset::new(
        AssetId::new(55)?,
        SourceId::new(1)?,
        [100_000, 100_000],
    )?))?;
    let transform = Transform::new([20.0, 20.0], [40.0, 40.0], 0.1, [false; 2])?;
    for id in [2, 1] {
        doc.apply(Command::AddObject {
            object: DocumentObject::image(ObjectId::new(id)?, AssetId::new(55)?, transform),
            index: doc.object_order().len(),
        })?;
    }
    fn consume(query: &impl DocumentQuery) -> Result<Vec<(ObjectId, AssetId)>, GeometryError> {
        Ok(query
            .objects_in_view(WorldRect::new(0.0, 0.0, 40.0, 40.0)?)
            .map(|data| (data.object_id, data.asset_id))
            .collect())
    }
    assert_eq!(
        consume(&doc)?,
        vec![
            (ObjectId::new(2)?, AssetId::new(55)?),
            (ObjectId::new(1)?, AssetId::new(55)?)
        ]
    );
    Ok(())
}
