//! Explicit bounded interface query, including LANs without a default gateway.
use std::{net::IpAddr, path::Path, process::Command, sync::atomic::AtomicBool, time::Duration};
use tack_assets::AssetError;
pub fn select_linux(bytes: &[u8]) -> Result<IpAddr, AssetError> {
    let interfaces: serde_json::Value = serde_json::from_slice(bytes)?;
    for interface in interfaces.as_array().ok_or("interface list")? {
        if interface["operstate"] != "UP"
            || interface["ifname"].as_str().is_some_and(|n| {
                n.starts_with("virbr") || n.starts_with("docker") || n.starts_with("br-")
            })
        {
            continue;
        }
        if let Some(addresses) = interface["addr_info"].as_array() {
            for a in addresses {
                if a["scope"] == "global"
                    && let Some(text) = a["local"].as_str()
                    && let Ok(ip) = text.parse::<IpAddr>()
                    && !ip.is_loopback()
                    && !ip.is_unspecified()
                    && !ip.is_multicast()
                {
                    return Ok(ip);
                }
            }
        }
    }
    Err("No active LAN address. Connect Ethernet/Wi-Fi or use a Tack server.".into())
}
pub fn local(work: &Path, cancel: &AtomicBool) -> Result<IpAddr, AssetError> {
    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("ip");
        command.args(["-j", "-4", "address", "show", "up"]);
        let bytes =
            crate::native_files::capture(command, work, cancel, 64 * 1024, Duration::from_secs(5))?;
        select_linux(&bytes)
    }
    #[cfg(windows)]
    {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile","-NonInteractive","-Command","Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -ne '127.0.0.1' -and $_.AddressState -eq 'Preferred' } | Select-Object -ExpandProperty IPAddress"]);
        let bytes =
            crate::native_files::capture(command, work, cancel, 64 * 1024, Duration::from_secs(5))?;
        for line in String::from_utf8(bytes)?.lines() {
            if let Ok(ip) = line.trim().parse::<IpAddr>()
                && !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
            {
                return Ok(ip);
            }
        }
        Err("No active LAN address".into())
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = (work, cancel);
        Err("Share on a Tack server on this platform".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_lan_does_not_require_a_route() {
        let fixture=br#"[{"ifname":"lo","operstate":"UP","addr_info":[{"scope":"host","local":"127.0.0.1"}]},{"ifname":"eth0","operstate":"UP","addr_info":[{"scope":"global","local":"192.168.42.2"}]}]"#;
        assert_eq!(
            select_linux(fixture).ok(),
            Some(IpAddr::from([192, 168, 42, 2]))
        );
        assert!(select_linux(b"[]").is_err());
    }
}
