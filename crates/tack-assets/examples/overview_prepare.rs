//! Standalone benchmark only. All filesystem work stays at startup/on workers.
use std::{
    fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use tack_assets::{AssetError, Board, Loader, OverviewPreparation, STAGE_NAMES};

fn main() -> Result<(), AssetError> {
    let started = Instant::now();
    let mut manifest = PathBuf::from("benchmark-data/mission0/manifest.json");
    let mut cache = PathBuf::from("benchmark-data/preparation-cache");
    let mut output = None;
    let mut workers = 2;
    let mut fraction = 1.0;
    let mut seconds = 300.0;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next().ok_or("missing argument value")?;
        match arg.as_str() {
            "--manifest" => manifest = value.into(),
            "--cache" => cache = value.into(),
            "--output" => output = Some(PathBuf::from(value)),
            "--workers" => workers = value.parse()?,
            "--fraction" => fraction = value.parse()?,
            "--seconds" => seconds = value.parse()?,
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if !(0.0..=1.0).contains(&fraction) || !(1.0..=600.0).contains(&seconds) {
        return Err("fraction must be 0..=1 and duration 1..=600".into());
    }
    let board = Board::read_manifest(&manifest)?;
    let mut loader = Loader::new(cache, 64 * 1024 * 1024, 512 * 1024 * 1024, workers)?;
    let mut preparation = OverviewPreparation::new(&board, started);
    let mut requests = Vec::new();
    let target = (board.objects.len() as f64 * fraction).ceil() as usize;
    let stop_reason = loop {
        loader.poll();
        preparation.observe(&loader);
        if preparation.ready() >= target {
            break "target";
        }
        if preparation.settled() {
            break "settled";
        }
        if started.elapsed().as_secs_f64() >= seconds {
            break "timeout";
        }
        preparation.remaining(&mut requests);
        loader.request(&requests);
        thread::sleep(Duration::from_millis(1));
    };
    let trigger_ms = started.elapsed().as_secs_f64() * 1000.0;
    let ready_at_trigger = preparation.ready();
    // Codec calls cannot be preempted. Drain explicitly rather than reporting a
    // partial state while a worker might still be renaming a cache file.
    loader.request(&[]);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        loader.poll();
        preparation.observe(&loader);
        if loader.stats().pending == 0 || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    let rss = fs::read_to_string("/proc/self/status").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("VmHWM:"))?
            .split_whitespace()
            .nth(1)?
            .parse::<u64>()
            .ok()
    });
    let report = serde_json::json!({
        "schema": 1, "workers": workers, "requested_fraction": fraction,
        "stop_reason": stop_reason,
        "trigger_ms": trigger_ms, "ready_at_trigger": ready_at_trigger,
        "pending_after_drain": loader.stats().pending,
        "worker_retention_peak_bytes": loader.stats().worker_retention_peak_bytes,
        "rss_high_water_kib": rss,
        "preparation": preparation.report(&loader, workers),
        "worker_stage_names": STAGE_NAMES, "worker_profiles": loader.profiles(),
        "worker_profile_dropped": loader.profile_dropped(),
    });
    if let Some(output) = output {
        fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("{}", report["preparation"]);
    if loader.stats().pending != 0 {
        return Err("preparation drain timed out".into());
    }
    Ok(())
}
