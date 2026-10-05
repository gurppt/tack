mod annotation_cli;
mod benchmark;
mod coverage;
mod navigation;
mod product_cli;
mod product_window;
mod report_output;
mod session;
mod spatial_cli;
mod supply_cli;
mod window;

use std::{path::PathBuf, time::Instant};
use tack_assets::AssetError;

#[derive(Clone)]
struct Options {
    manifest: PathBuf,
    cache: PathBuf,
    output: Option<PathBuf>,
    scenario: Option<String>,
    seconds: f64,
    headless: bool,
    workers: usize,
    prefetch: String,
    prepare: bool,
}

impl Options {
    fn parse() -> Result<Self, AssetError> {
        let mut options = Self {
            manifest: "benchmark-data/mission0/manifest.json".into(),
            cache: "benchmark-data/display-cache".into(),
            output: None,
            scenario: None,
            seconds: 12.0,
            headless: false,
            workers: 2,
            prefetch: "none".into(),
            prepare: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            if arg == "--prepare-overview" {
                options.prepare = true;
                continue;
            }
            if arg == "--headless" {
                options.headless = true;
                continue;
            }
            if arg == "--help" {
                println!(
                    "Product: tack-app (new empty board) | new [UNUSED_PATH.tack] | FILE.tack | create OUTPUT.tack --linked|--embedded IMAGE... | open FILE.tack | inspect FILE.tack | repair INPUT.tack OUTPUT.tack | query-scale [REPORT.json]\nBenchmark-only: tack-app [--manifest PATH] [--cache PATH] [--scenario NAME] [--seconds 12] [--output PATH] [--headless] [--workers 1|2|4] [--prefetch none|symmetric|directional] [--prepare-overview (benchmark only)]\nScenarios: {}\nF10 menu; Ctrl+N new; Ctrl+O open; Ctrl+I import; Ctrl+V paste; Ctrl+S save; Ctrl+Shift+S Save As; Ctrl+Shift+R relink; Ctrl+comma preferences. Navigation: middle drag or Alt + left drag; wheel zoom.",
                    navigation::SCENARIOS.join(", ")
                );
                std::process::exit(0);
            }
            let value = args.next().ok_or("option missing its value")?;
            match arg.as_str() {
                "--manifest" => options.manifest = value.into(),
                "--cache" => options.cache = value.into(),
                "--output" => options.output = Some(value.into()),
                "--scenario" => options.scenario = Some(value),
                "--seconds" => options.seconds = value.parse()?,
                "--workers" => options.workers = value.parse()?,
                "--prefetch" => options.prefetch = value,
                _ => return Err(format!("unknown option {arg}").into()),
            }
        }
        if !(1.0..=120.0).contains(&options.seconds) {
            return Err("duration must be 1..=120 seconds".into());
        }
        if ![1, 2, 4].contains(&options.workers)
            || !["none", "symmetric", "directional"].contains(&options.prefetch.as_str())
        {
            return Err("workers must be 1, 2 or 4 and prefetch none|symmetric|directional".into());
        }
        if options
            .scenario
            .as_ref()
            .is_some_and(|s| !navigation::SCENARIOS.contains(&s.as_str()))
        {
            return Err("unknown benchmark scenario".into());
        }
        if options.headless && options.scenario.is_none() {
            return Err("headless mode requires --scenario".into());
        }
        if options.prepare && options.scenario.is_none() {
            return Err("--prepare-overview requires a scripted benchmark scenario".into());
        }
        Ok(options)
    }
}

fn main() -> Result<(), AssetError> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--font-license")
    {
        print!(
            "{}\n{}\n{}",
            include_str!("../../../assets/pixel-font/COPYRIGHT.txt"),
            include_str!("../../../assets/pixel-font/OFL-1.1.txt"),
            include_str!("../../../assets/ui-font/LICENSE")
        );
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tack=info,wgpu=warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let started = Instant::now();
    let mut product_args = std::env::args_os().skip(1);
    if product_args.len() == 0 {
        return product_window::run_new(Vec::new(), started);
    }
    let first = std::env::args_os().nth(1);
    if first.as_ref().is_some_and(|a| {
        std::path::Path::new(a)
            .extension()
            .is_some_and(|e| e == "tack")
    }) {
        return product_window::run(std::env::args_os().skip(1).collect(), started);
    }
    if let Some(command) = product_args.next()
        && let Some(command) = command.to_str()
        && [
            "create",
            "new",
            "repair",
            "inspect",
            "open",
            "query-scale",
            "spatial-scale",
            "supply-scale",
            "annotation-scale",
        ]
        .contains(&command)
    {
        return product_cli::run(command, product_args.collect(), started);
    }
    let options = Options::parse()?;
    report_output::preflight(options.output.as_deref(), None)?;
    if options.headless {
        benchmark::headless(options, started)
    } else {
        window::run(options, started)
    }
}
