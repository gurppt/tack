use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
#[derive(Clone, Copy, Debug, Default)]
pub struct ClientStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}
#[derive(Default)]
pub(super) struct Counters {
    sent: AtomicU64,
    received: AtomicU64,
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
}
impl Counters {
    pub fn snapshot(&self) -> ClientStats {
        ClientStats {
            bytes_sent: self.sent.load(Ordering::Relaxed),
            bytes_received: self.received.load(Ordering::Relaxed),
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
        }
    }
}
pub(super) struct Observed<'a> {
    pub socket: &'a mut TcpStream,
    pub counters: Arc<Counters>,
}
impl Read for Observed<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.socket.read(bytes)?;
        self.counters
            .received
            .fetch_add(count as u64, Ordering::Relaxed);
        Ok(count)
    }
}
impl Write for Observed<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.socket.write(bytes)?;
        self.counters
            .sent
            .fetch_add(count as u64, Ordering::Relaxed);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.socket.flush()
    }
}
impl crate::publish::MessageIo for Observed<'_> {
    fn message_sent(&self) {
        self.counters.messages_sent.fetch_add(1, Ordering::Relaxed);
    }
    fn message_received(&self) {
        self.counters
            .messages_received
            .fetch_add(1, Ordering::Relaxed);
    }
}
