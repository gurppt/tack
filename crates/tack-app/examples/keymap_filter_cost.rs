//! Finite developer measurement over the canonical catalog, no runtime index.
use std::{hint::black_box, time::Instant};
fn main() -> Result<(), tack_assets::AssetError> {
    let profile = tack_app::preferences::Preferences::defaults()?;
    let keymap = profile.keymap()?;
    let mut rows = Vec::new();
    for query in ["", "Undo", "pointer", "ctrl+z", "View", "NO-MATCH"] {
        let start = Instant::now();
        let iterations = 10000;
        for _ in 0..iterations {
            black_box(tack_app::local_ui::filtered_actions(
                black_box(query),
                black_box(&keymap),
            ));
        }
        rows.push(
            serde_json::json!({"query":query,"actions":tack_app::actions::Action::ALL.len(),
            "ns_per_scan":start.elapsed().as_nanos()/iterations,"iterations":iterations}),
        );
    }
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
