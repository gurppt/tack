mod apply;
mod network;
mod package;
mod transaction;
use tack_update::{Channel, Error, Manifest};
fn run() -> Result<(), Error> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "--build-info")
    {
        println!(
            "{}",
            serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"git_sha":env!("TACK_GIT_SHA"),"channel":env!("TACK_BUILD_CHANNEL"),"protocol_major":tack_update::PROTOCOL_MAJOR})
        );
        return Ok(());
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let response=match args.first().and_then(|s|s.to_str()){
        Some("check") if args.len()==3=>{
            let channel=match args[1].to_str(){Some("dev")=>Channel::Dev,Some("stable")=>Channel::Stable,_=>return Err("Invalid update channel".into())};
            network::check(channel,args[2].to_str().ok_or("Invalid version")?)?
        }
        Some("stage") if args.len()==3=>{
            let manifest=Manifest::parse(&tack_update::read_small(std::path::Path::new(&args[1]),tack_update::MAX_MANIFEST)?)?;
            package::stage(&manifest,std::path::Path::new(&args[2]))?
        }
        Some("recover") if args.len()==2 => {
            let recovered = transaction::recover(std::path::Path::new(&args[1]))?;
            println!("{}", serde_json::json!({"recovered":recovered}));
            return Ok(());
        }
        Some("apply") if args.len()==4=>{
            let pid:u32=args[3].to_str().ok_or("Invalid PID")?.parse()?;
            apply::apply(std::path::Path::new(&args[1]),std::path::Path::new(&args[2]),pid)?;
            return Ok(());
        }
        _=>return Err("Usage: tack-updater check <dev|stable> <version> | stage <manifest> <install> | apply <stage> <install> <pid> | recover <install>".into())
    };
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Update: {e}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod test_support;

#[cfg(all(test, target_os = "linux"))]
mod apply_tests;
