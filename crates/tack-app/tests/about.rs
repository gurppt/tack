use std::time::{Duration, Instant};
use tack_app::{
    about,
    local_worker::{LocalUpdate, LocalWorker, Operation},
    ui_theme::Theme,
};
use tack_assets::AssetError;
use tack_core::Camera;
const IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/about.png"));

#[test]
fn compact_package_and_corrupt_artwork_have_bounded_results() -> Result<(), AssetError> {
    let image = about::decode_image(IMAGE)?;
    assert_eq!([image.width, image.height], [207, 224]);
    assert_eq!(image.rgba.len(), 185472);
    assert_eq!(about::METADATA.version, env!("CARGO_PKG_VERSION"));
    assert!(!about::METADATA.author.is_empty());
    for invalid in [&[][..], b"broken png", &IMAGE[..32]] {
        assert!(about::decode_image(invalid).is_err());
    }
    assert!(tack_assets::decode_ui_png(&vec![0; 128 * 1024 + 1]).is_err());
    Ok(())
}

#[test]
fn every_metadata_row_and_close_fit_800_by_600_at_both_scales() {
    for scale in [1., 2.] {
        let mut camera = Camera::new([800, 600]);
        camera.set_ui_scale(scale);
        let layout = about::Layout::new(&camera);
        for [x, y, w, h] in [layout.panel, layout.image, layout.text, layout.close] {
            assert!(x >= 0. && y >= 0. && w > 0. && h > 0.);
            assert!((x + w) * scale <= 800. && (y + h) * scale <= 600.);
        }
        let rows = about::text_rows(
            (layout.text[2] / 8.).floor() as usize,
            Theme::default().palette(),
        );
        assert!(rows.len() as f64 * 16. <= layout.text[3]);
        assert!((layout.image[2] / layout.image[3] - 587. / 635.).abs() < 0.003);
        assert!(layout.close_hit([
            (layout.close[0] + 32.) * scale,
            (layout.close[1] + 10.) * scale
        ]));
        assert!(!layout.close_hit([0., 0.]));
        for (text, _) in rows {
            let cells: usize = text.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
            assert!(cells as f64 * 8. <= layout.text[2]);
        }
    }
    assert_eq!(
        about::wrap("Author: Captain Cool - Suspicious Sausage Records", 18),
        ["Author: Captain", "Cool - Suspicious", "Sausage Records"]
    );
    let url = "https://github.com/gurppt/tack";
    assert_eq!(about::wrap(url, 18).join(""), url);
}

#[test]
fn repeated_on_demand_worker_finishes_and_drops_its_payload() -> Result<(), AssetError> {
    let mut worker = LocalWorker::default();
    assert!(!worker.active());
    for ticket in 1..=20 {
        worker.start(Operation::About { ticket }, || {})?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut received = false;
        while worker.active() {
            for update in worker.poll() {
                match update {
                    LocalUpdate::About {
                        ticket: actual,
                        result,
                    } => {
                        assert_eq!(actual, ticket);
                        assert_eq!(result?.rgba.len(), 185472);
                        received = true;
                    }
                    LocalUpdate::Done(result) => result?,
                    _ => return Err("unexpected worker result".into()),
                }
            }
            if Instant::now() >= deadline {
                return Err("About worker failed to retire".into());
            }
            std::thread::yield_now();
        }
        assert!(received);
    }
    assert_eq!(worker.operations, 20);
    Ok(())
}
