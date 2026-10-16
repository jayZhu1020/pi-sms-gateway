//! Exercise the complete probe/exchange/parser chain without a physical modem.
use sim7600_probe::{SimStatus, probe_sim};
use std::io::{self, Cursor, Read, Write};
use std::time::{Duration, Instant};

struct SimulatedModem {
    received: Vec<u8>,
    reply: Cursor<Vec<u8>>,
}
impl Write for SimulatedModem {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.received.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("unexpected flush")
    }
}
impl Read for SimulatedModem {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        assert_eq!(self.received, b"AT+CPIN?\r");
        self.reply.read(bytes)
    }
}
fn modem(reply: &str) -> SimulatedModem {
    SimulatedModem {
        received: Vec::new(),
        reply: Cursor::new(reply.as_bytes().to_vec()),
    }
}
#[test]
fn recognizes_supported_states_and_counts_notifications() {
    for (text, expected) in [
        ("READY", SimStatus::Ready),
        ("SIM PIN", SimStatus::PinRequired),
        ("SIM PUK", SimStatus::PukRequired),
    ] {
        let reply = format!("AT+CPIN?\r\n+CMTI: 1\r\n+CPIN: {text}\r\nOK\r\n");
        let (transport, result) =
            probe_sim(modem(&reply), Instant::now() + Duration::from_secs(2)).unwrap();
        assert_eq!(result.status, expected);
        assert_eq!(result.notifications, 1);
        assert_eq!(transport.received, b"AT+CPIN?\r");
    }
}
#[test]
fn rejects_missing_unknown_duplicate_and_failed_replies() {
    for reply in [
        "OK\r\n",
        "+CPIN: UNKNOWN\r\nOK\r\n",
        "+CPIN: READY\r\n+CPIN: READY\r\nOK\r\n",
        "ERROR\r\n",
        "+CPIN: READY\r\n",
    ] {
        assert!(probe_sim(modem(reply), Instant::now() + Duration::from_secs(2)).is_err());
    }
    assert!(probe_sim(modem("OK\r\n"), Instant::now()).is_err());
}
