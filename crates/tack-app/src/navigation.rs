//! Deterministic camera traces. Speeds are fixed independently of loader progress.
use tack_assets::BenchmarkBoard;
use tack_core::{Camera, GeometryError};

pub const SCENARIOS: [&str; 11] = [
    "cold",
    "warm",
    "pan",
    "zoom",
    "pressure",
    "pan-slow",
    "pan-normal",
    "pan-fast",
    "zoom-traverse",
    "scan",
    "board-tour",
];

fn adjacent(board: &BenchmarkBoard, distance: f64) -> [f64; 2] {
    let first = board.objects[0].rect;
    let max_x = board
        .objects
        .iter()
        .map(|o| o.rect.x)
        .fold(first.x, f64::max);
    let max_y = board
        .objects
        .iter()
        .map(|o| o.rect.y)
        .fold(first.y, f64::max);
    let horizontal = (max_x - first.x).max(6600.0);
    let row_length = horizontal + 5100.0;
    let rows = ((max_y - first.y) / 5100.0).floor() as usize + 1;
    let path = distance % (rows as f64 * row_length);
    let row = (path / row_length).floor() as usize;
    let along = path % row_length;
    let x = if row.is_multiple_of(2) {
        along.min(horizontal)
    } else {
        horizontal - along.min(horizontal)
    };
    let y = row as f64 * 5100.0 + (along - horizontal).clamp(0.0, 5100.0);
    [
        first.x + first.width / 2.0 + x,
        first.y + first.height / 2.0 + y,
    ]
}

pub fn apply(
    camera: &mut Camera,
    board: &BenchmarkBoard,
    scenario: &str,
    seconds: f64,
) -> Result<(), GeometryError> {
    match scenario {
        "pan" | "pressure" => {
            let index = if scenario == "pan" {
                (seconds * 20.0) as usize * 37
            } else {
                (seconds / 2.0) as usize * 17
            } % board.objects.len();
            let r = board.objects[index].rect;
            camera.set_view(
                [r.x + r.width / 2.0, r.y + r.height / 2.0],
                if scenario == "pan" { 0.1 } else { 0.6 },
            )
        }
        "zoom" => camera.set_view(
            [6600.0, 5100.0],
            0.003 * ((seconds * 2.0).sin() * 0.5 + 0.5).mul_add(6.0, 0.0).exp(),
        ),
        "pan-slow" | "pan-normal" | "pan-fast" => {
            let speed = match scenario {
                "pan-slow" => 3200.0,
                "pan-normal" => 12800.0,
                _ => 38400.0,
            };
            camera.set_view(adjacent(board, seconds * speed), 0.1)
        }
        "zoom-traverse" | "scan" => {
            let phase = (seconds * std::f64::consts::TAU / 8.0).cos();
            let zoom = 0.006 * (100.0_f64.ln() * (1.0 - phase) / 2.0).exp();
            let center = if scenario == "scan" {
                adjacent(board, seconds * 6400.0)
            } else {
                [6600.0, 5100.0]
            };
            camera.set_view(center, zoom)
        }
        "board-tour" => {
            // Overview spans the board width. Traverse vertically without loading
            // originals into RAM in advance; all supply uses the ordinary loader.
            let max_y = board
                .objects
                .iter()
                .map(|o| o.rect.y + o.rect.height)
                .fold(0.0, f64::max);
            let max_x = board
                .objects
                .iter()
                .map(|o| o.rect.x + o.rect.width)
                .fold(0.0, f64::max);
            let phase = seconds / 12.0 % 2.0;
            let fraction = if phase <= 1.0 { phase } else { 2.0 - phase };
            camera.set_view(
                [max_x / 2.0, 4500.0 + fraction * (max_y - 9000.0).max(0.0)],
                0.006,
            )
        }
        _ => camera.set_view([6600.0, 5100.0], 0.1),
    }
}
