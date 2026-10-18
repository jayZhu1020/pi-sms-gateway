//! Independent modem fixtures through the real writer/reader/semantic pipeline.
use sim7600_probe::{SmsFormat, probe_sms_storage};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::{self, Cursor, Read, Write};
use std::rc::Rc;
use std::time::{Duration, Instant};

type Script = VecDeque<(&'static [u8], Cursor<Vec<u8>>)>;
struct Modem {
    script: Script,
    sent: usize,
    writes: Rc<RefCell<Vec<u8>>>,
}
impl Write for Modem {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let expected = self.script.front().expect("unexpected command").0;
        let count = bytes.len().min(2);
        assert_eq!(&bytes[..count], &expected[self.sent..self.sent + count]);
        self.sent += count;
        self.writes.borrow_mut().extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Read for Modem {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let (expected, reply) = self.script.front_mut().expect("unexpected read");
        assert_eq!(self.sent, expected.len(), "read before full command");
        let count = reply.read(bytes)?;
        if count != 0 && reply.position() == reply.get_ref().len() as u64 {
            self.script.pop_front();
            self.sent = 0;
        }
        Ok(count)
    }
}
fn modem(format: &str, storage: &str) -> Modem {
    Modem {
        script: [
            (
                b"AT+CMGF?\r".as_slice(),
                Cursor::new(format.as_bytes().to_vec()),
            ),
            (
                b"AT+CPMS?\r".as_slice(),
                Cursor::new(storage.as_bytes().to_vec()),
            ),
        ]
        .into(),
        sent: 0,
        writes: Rc::new(RefCell::new(Vec::new())),
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}
const STORAGE: &str = "+CPMS: \"SM\",2,20,\"ME\",9,9,\"SM\",2,20\r\nOK\r\n";

#[test]
fn queries_only_format_and_storage_preserving_roles_and_notifications() {
    let format = "AT+CMGF?\r\n+CMTI: \"SM\",7\r\n+CMGF: 0\r\nOK\r\n";
    let storage = format!("AT+CPMS?\r\n+FUTURE: private\r\n{STORAGE}");
    let (transport, result) = probe_sms_storage(modem(format, &storage), deadline()).unwrap();
    assert_eq!(*transport.writes.borrow(), b"AT+CMGF?\rAT+CPMS?\r");
    assert!(transport.script.is_empty());
    assert_eq!(result.format, SmsFormat::Pdu);
    assert_eq!(result.notifications, 2);
    assert_eq!(result.storage.read_delete.storage.to_string(), "SIM");
    assert_eq!(
        (
            result.storage.read_delete.used,
            result.storage.read_delete.total
        ),
        (2, 20)
    );
    assert_eq!(result.storage.write_send.storage.to_string(), "modem");
    assert_eq!(
        (
            result.storage.write_send.used,
            result.storage.write_send.total
        ),
        (9, 9)
    );
    assert_eq!(result.storage.receive.storage.to_string(), "SIM");
}

#[test]
fn text_mode_is_reported_without_changing_format() {
    let (transport, result) =
        probe_sms_storage(modem("+CMGF: 1\r\nOK\r\n", STORAGE), deadline()).unwrap();
    assert_eq!(result.format, SmsFormat::Text);
    assert_eq!(*transport.writes.borrow(), b"AT+CMGF?\rAT+CPMS?\r");
}

#[test]
fn invalid_first_reply_stops_before_storage_query_and_hides_raw_data() {
    for reply in [
        "OK\r\n",
        "ERROR\r\n",
        "+CMGF: PRIVATE\r\nOK\r\n",
        "+CMGF: 0\r\n+CMGF: 1\r\nOK\r\n",
        "",
    ] {
        let transport = modem(reply, STORAGE);
        let writes = transport.writes.clone();
        let error = probe_sms_storage(transport, deadline()).err().unwrap();
        assert!(!error.to_string().contains("PRIVATE"));
        assert_eq!(*writes.borrow(), b"AT+CMGF?\r");
    }
}

#[test]
fn invalid_storage_reply_cannot_return_a_partial_snapshot() {
    for reply in [
        "OK\r\n",
        "+CMS ERROR: 500\r\n",
        "+CPMS: PRIVATE\r\nOK\r\n",
        "+CPMS: \"SM\",0,1,\"SM\",0,1,\"SM\",0,1\r\n+CPMS: \"SM\",0,1,\"SM\",0,1,\"SM\",0,1\r\nOK\r\n",
        "",
    ] {
        let error = probe_sms_storage(modem("+CMGF: 0\r\nOK\r\n", reply), deadline())
            .err()
            .unwrap();
        assert!(!error.to_string().contains("PRIVATE"));
    }
    let transport = modem("+CMGF: 0\r\nOK\r\n", STORAGE);
    let writes = transport.writes.clone();
    assert!(probe_sms_storage(transport, Instant::now()).is_err());
    assert!(writes.borrow().is_empty());
}
