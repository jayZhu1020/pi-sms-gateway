//! Exercise the real writer/reader/parser chain through an in-memory modem peer.
use at_exchange::{AtExchange, ExchangeError};
use at_reply_reader::ReplyReadError;
use at_response::{ResponseError, ResponseEvent};
use std::io::{self, Cursor, Read, Write};
use std::time::{Duration, Instant};

struct SimulatedModem {
    received: Vec<u8>,
    replies: Cursor<&'static [u8]>,
}
impl Read for SimulatedModem {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        assert_eq!(
            self.received, b"AT+CPIN?\r",
            "reply read before command finished"
        );
        self.replies.read(bytes)
    }
}
impl Write for SimulatedModem {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(2);
        self.received.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("unexpected flush")
    }
}
fn modem(replies: &'static [u8]) -> SimulatedModem {
    SimulatedModem {
        received: Vec::new(),
        replies: Cursor::new(replies),
    }
}

#[test]
fn complete_exchange_preserves_notifications_and_returns_connection() {
    let now = Instant::now();
    let mut exchange = AtExchange::new(
        modem(b"+CMTI: 1\r\n+CPIN: READY\r\nOK\r\ntail"),
        "AT+CPIN?",
        "+CPIN:",
        now + Duration::from_secs(1),
    )
    .unwrap();
    let mut events = Vec::new();
    for _ in 0..100 {
        if let Some(event) = exchange.poll(now).unwrap() {
            let complete = matches!(event, ResponseEvent::Complete);
            events.push(event);
            if complete {
                break;
            }
        }
    }
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], ResponseEvent::Notification(s) if s == "+CMTI: 1"));
    assert!(matches!(&events[1], ResponseEvent::Data(s) if s == "+CPIN: READY"));
    assert!(matches!(events[2], ResponseEvent::Complete));
    let mut transport = exchange.finish().ok().unwrap();
    let mut tail = String::new();
    transport.read_to_string(&mut tail).unwrap();
    assert_eq!(tail, "tail");
}

#[test]
fn deadline_is_shared_between_writing_and_reading() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(1);
    let mut exchange = AtExchange::new(modem(b"OK\r\n"), "AT+CPIN?", "+CPIN:", deadline).unwrap();
    for _ in 0..5 {
        assert!(exchange.poll(now).unwrap().is_none());
    }
    assert!(matches!(
        exchange.poll(deadline),
        Err(ExchangeError::Reply(ReplyReadError::Response(
            ResponseError::TimedOut
        )))
    ));
    assert!(matches!(exchange.poll(now), Err(ExchangeError::Closed)));
    assert!(exchange.finish().is_err());
    let mut exchange = AtExchange::new(modem(b""), "AT+CPIN?", "+CPIN:", deadline).unwrap();
    assert!(
        matches!(exchange.poll(deadline), Err(ExchangeError::Write(e)) if e.kind() == io::ErrorKind::TimedOut)
    );
    assert!(matches!(exchange.poll(now), Err(ExchangeError::Closed)));
    assert!(exchange.finish().is_err());
    assert!(AtExchange::new(modem(b""), "AT", "", deadline).is_err());
}
