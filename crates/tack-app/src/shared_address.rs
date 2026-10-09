//! Client-side syntax only. The existing transport accepts numeric IP endpoints.
use std::net::SocketAddr;
use tack_assets::AssetError;
use tack_shared::WireId;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedAddress {
    pub server: SocketAddr,
    pub board: WireId,
}
impl SharedAddress {
    pub fn parse(text: &str) -> Result<Self, AssetError> {
        let text = text.trim();
        if text.len() > 256 {
            return Err("shared address exceeds 256 bytes".into());
        }
        let (server, board) = if let Some(uri) = text.strip_prefix("tack://") {
            uri.split_once('/').ok_or("Use tack://IP:PORT/BOARD_ID")?
        } else {
            text.split_once(char::is_whitespace)
                .ok_or("Use IP:PORT BOARD_ID or tack://IP:PORT/BOARD_ID")?
        };
        let server: SocketAddr = server
            .parse()
            .map_err(|_| "Use a numeric IP and port (IPv6 in brackets)")?;
        if server.port() == 0 || server.ip().is_unspecified() || server.ip().is_multicast() {
            return Err("Use a specific unicast IP and port 1..65535".into());
        }
        Ok(Self {
            server,
            board: WireId::parse(board.trim())?,
        })
    }
    pub fn canonical(&self) -> String {
        format!("tack://{}/{}", self.server, self.board)
    }
}
