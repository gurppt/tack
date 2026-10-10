//! UI capability state: no protocol, transport or client types cross this seam.
#[derive(Clone, Copy)]
pub enum UiConnection {
    Online,
    Connecting,
    Offline,
}
