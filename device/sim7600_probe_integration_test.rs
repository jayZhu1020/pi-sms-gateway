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

// Refuse reads until the expected command has been completely written. The next
// command is accepted only after the previous scripted reply has been consumed.
struct NetworkModem {
    step: usize,
    command: Vec<u8>,
    replies: [Cursor<Vec<u8>>; 2],
    writes: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
}
impl Write for NetworkModem {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.command == self.expected()
            && self.replies[self.step].position() == self.replies[self.step].get_ref().len() as u64
        {
            self.step += 1;
            self.command.clear();
        }
        let count = bytes.len().min(2);
        self.command.extend_from_slice(&bytes[..count]);
        assert!(self.expected().starts_with(&self.command));
        self.writes.borrow_mut().extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("unexpected flush")
    }
}
impl NetworkModem {
    fn expected(&self) -> &[u8] {
        [b"AT+CREG?\r".as_slice(), b"AT+CEREG?\r".as_slice()][self.step]
    }
}
impl Read for NetworkModem {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        assert_eq!(self.command, self.expected());
        self.replies[self.step].read(bytes)
    }
}
fn network_modem(first: &str, second: &str) -> NetworkModem {
    NetworkModem {
        step: 0,
        command: Vec::new(),
        replies: [first, second].map(|s| Cursor::new(s.as_bytes().to_vec())),
        writes: Default::default(),
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}

#[test]
fn network_queries_share_transport_and_keep_location_private() {
    use sim7600_probe::{RegistrationStatus, probe_network};
    let first = "AT+CREG?\r\n+CMTI: 1\r\n+CREG: 2,5,\"00A1\",\"00112233\",7\r\nOK\r\n";
    let second = "AT+CEREG?\r\n+OTHER: private\r\n+CEREG: 2,1,\"00B2\",\"1234ABCD\",7\r\nOK\r\n";
    let (transport, result) = probe_network(network_modem(first, second), deadline()).unwrap();
    assert_eq!(result.circuit_switched, RegistrationStatus::Roaming);
    assert_eq!(result.eps, RegistrationStatus::Home);
    assert_eq!(result.notifications, 2);
    assert_eq!(*transport.writes.borrow(), b"AT+CREG?\rAT+CEREG?\r");
    let debug = format!("{result:?}");
    assert!(!debug.contains("00A1") && !debug.contains("1234ABCD") && !debug.contains("private"));
}

#[test]
fn supports_documented_registration_states_without_claiming_delivery() {
    use sim7600_probe::{RegistrationStatus::*, probe_network};
    for (code, expected) in [
        (0, NotRegistered),
        (1, Home),
        (2, Searching),
        (3, Denied),
        (4, Unknown),
        (5, Roaming),
    ] {
        let reply = format!("+CREG: 0,{code}\r\nOK\r\n");
        let (_, result) =
            probe_network(network_modem(&reply, "+CEREG: 0,4\r\nOK\r\n"), deadline()).unwrap();
        assert_eq!(result.circuit_switched, expected);
        assert_eq!(result.eps, Unknown);
    }
    for (code, expected) in [
        (0, NotRegistered),
        (1, Home),
        (2, Searching),
        (3, Denied),
        (4, Unknown),
        (5, Roaming),
        (6, SmsOnlyHome),
        (7, SmsOnlyRoaming),
        (8, EmergencyOnly),
    ] {
        let reply = format!("+CEREG: 1,{code}\r\nOK\r\n");
        let (_, result) =
            probe_network(network_modem("+CREG: 0,0\r\nOK\r\n", &reply), deadline()).unwrap();
        assert_eq!(result.eps, expected);
    }
}

#[test]
fn malformed_or_ambiguous_registration_stops_before_next_command() {
    use sim7600_probe::probe_network;
    for reply in [
        "OK\r\n",
        "ERROR\r\n",
        "+CME ERROR: 13\r\n",
        "+CREG: 0,1\r\n",
        "+CREG: 0,1\r\n+CREG: 0,1\r\nOK\r\n",
        "+CREG: 1\r\n+CREG: 0,1\r\nOK\r\n",
        "+CREG: 0,6\r\nOK\r\n",
        "+CREG: 3,1\r\nOK\r\n",
        "+CREG: 0,1,junk\r\nOK\r\n",
        "+CREG: -1,1\r\nOK\r\n",
        "+CREG: 2,1,\"12G4\",\"1234\"\r\nOK\r\n",
        "+CREG: 2,1,\"12345\",\"1234\"\r\nOK\r\n",
        "+CREG: 2,1,\"1234\",\"123456789\"\r\nOK\r\n",
        "+CREG: 2,1,\"1234\",\"1234\",99\r\nOK\r\n",
        "+CREG: 0,255\r\nOK\r\n",
    ] {
        let transport = network_modem(reply, "");
        let writes = transport.writes.clone();
        assert!(
            probe_network(transport, deadline()).is_err(),
            "accepted {reply:?}"
        );
        assert_eq!(*writes.borrow(), b"AT+CREG?\r");
    }
}

#[test]
fn second_query_failure_and_shared_expiry_fail_the_snapshot() {
    use sim7600_probe::probe_network;
    for reply in [
        "",
        "OK\r\n",
        "+CEREG: 0,9\r\nOK\r\n",
        "+CME ERROR: 13\r\n",
        "+CEREG: 0,1\r\n+CEREG: 0,1\r\nOK\r\n",
    ] {
        assert!(probe_network(network_modem("+CREG: 0,1\r\nOK\r\n", reply), deadline()).is_err());
    }
    let transport = network_modem("", "");
    let writes = transport.writes.clone();
    assert!(probe_network(transport, Instant::now()).is_err());
    assert!(writes.borrow().is_empty());
}
