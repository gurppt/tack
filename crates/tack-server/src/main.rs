use tack_server::{ServerConfig, serve};
fn main() {
    if let Err(error) = run() {
        eprintln!("tack-server: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut config = ServerConfig::default();
    let mut ready = None;
    let mut snapshot = None;
    let mut board = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--managed-ready" => {
                ready = Some(std::path::PathBuf::from(
                    arguments.next().ok_or("ready file required")?,
                ))
            }
            "--managed-snapshot" => {
                snapshot = Some(std::path::PathBuf::from(
                    arguments.next().ok_or("snapshot file required")?,
                ))
            }
            "--managed-board" => {
                board = Some(
                    tack_shared::WireId::parse(&arguments.next().ok_or("board required")?)
                        .map_err(|e| e.to_string())?,
                )
            }
            "--listen" => config.listen = arguments.next().ok_or("--listen needs IP:port")?,
            "--root" => config.root = arguments.next().ok_or("--root needs directory")?.into(),
            "--asset-quota" => {
                config.asset_quota = arguments
                    .next()
                    .ok_or("--asset-quota needs bytes")?
                    .parse()
                    .map_err(|_| "invalid asset quota")?
            }
            "--help" | "-h" => {
                println!(
                    "tack-server --listen 127.0.0.1:7337 --root DIRECTORY [--asset-quota BYTES]\nTrusted LAN only; no authentication or TLS. Local Tack never starts this process."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    if ready.is_some() || snapshot.is_some() || board.is_some() {
        config.managed = Some(tack_server::managed::Config {
            ready: ready.ok_or("managed ready missing")?,
            snapshot: snapshot.ok_or("managed snapshot missing")?,
            board: board.ok_or("managed board missing")?,
        });
    }
    serve(config)
}
