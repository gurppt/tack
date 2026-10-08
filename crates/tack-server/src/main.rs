use tack_server::{ServerConfig, serve};
fn main() {
    if let Err(error) = run() {
        eprintln!("tack-server: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut config = ServerConfig::default();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
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
    serve(config)
}
