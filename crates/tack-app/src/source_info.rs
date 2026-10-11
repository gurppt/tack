//! Read-only metadata formatting: no filesystem, decoding, cache lookup or worker.
use tack_assets::{AssetError, SourceState};
use tack_core::{Document, DocumentQuery, ObjectId, SourceLocation};
pub fn rows(
    doc: &Document,
    id: ObjectId,
    state: Option<SourceState>,
    encoded_bytes: Option<u64>,
) -> Result<Vec<String>, AssetError> {
    let image = doc
        .object_render_data(id)
        .ok_or("Information requires one image")?;
    let asset = doc.asset(image.asset_id).ok_or("Image asset unavailable")?;
    let source = doc
        .source(asset.source_id())
        .ok_or("Image source unavailable")?;
    let [w, h] = asset.pixel_size();
    let mut rows = vec![
        format!("Source {w} x {h} px"),
        format!(
            "Display {:.0} x {:.0}",
            image.transform.size()[0],
            image.transform.size()[1]
        ),
        format!(
            "Scale {:.1}%",
            image.transform.size()[0] / (f64::from(w) * image.crop.uv_rect()[2]) * 100.
        ),
        format!(
            "Storage: {}",
            if matches!(source.location(), SourceLocation::Embedded) {
                "Embedded"
            } else {
                "Linked"
            }
        ),
        format!(
            "Status: {}",
            state.map_or("Not observed".into(), |s| format!("{s:?}"))
        ),
        format!(
            "Original: {}",
            encoded_bytes
                .or(source.fingerprint().map(|f| f.size))
                .map_or("Size not recorded".into(), |n| format!("{n} encoded bytes"))
        ),
        format!("Sampling: {:?}", image.filtering),
        format!("Opacity: {:.0}%", image.opacity.value() * 100.),
        format!("Crop: {:?}", image.crop),
        format!("Source revision: {}", source.revision()),
    ];
    if let SourceLocation::Linked(path) = source.location() {
        rows.push(format!(
            "Path: {}",
            path.to_native()
                .map_or("Foreign platform path".into(), |p| p.display().to_string())
        ));
    }
    Ok(rows)
}

/// Source identity palette shared by chrome, preferences and selection.
pub fn storage_color(embedded: bool) -> crate::ui_theme::Color {
    tack_core::Color(if embedded {
        [248, 92, 218, 255]
    } else {
        [44, 224, 202, 255]
    })
    .rgba(tack_core::Opacity::OPAQUE)
}
pub fn missing_color() -> crate::ui_theme::Color {
    tack_core::Color([255, 72, 84, 255]).rgba(tack_core::Opacity::OPAQUE)
}
pub fn color(location: &SourceLocation, state: Option<SourceState>) -> crate::ui_theme::Color {
    if matches!(
        state,
        Some(SourceState::Missing | SourceState::Foreign | SourceState::Unavailable)
    ) && matches!(location, SourceLocation::Linked(_))
    {
        missing_color()
    } else {
        storage_color(matches!(location, SourceLocation::Embedded))
    }
}
