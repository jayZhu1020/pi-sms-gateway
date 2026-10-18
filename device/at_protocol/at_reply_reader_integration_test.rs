//! Combine the real byte decoder and reply tracker through the reader's public API.
use at_command::{Command, Notification, Reply, SimStatus};
use at_reply_reader::{AtReplyReader, ReplyReadError};
use at_response::{ResponseError, ResponseEvent};
use std::io::{self, Cursor, Read};
use std::time::{Duration, Instant};

#[test]
fn routes_lines_and_leaves_bytes_after_completion_unread() {
    let now = Instant::now();
    let input = b"AT+CPIN?\r\n+CPIN: READY\r\n+CMTI: 1\r\nOK\r\n+CMTI: 2\r\n";
    let mut reader = AtReplyReader::new(
        Cursor::new(input),
        Command::QuerySimReadiness,
        now + Duration::from_secs(1),
    );
    let mut events = Vec::new();
    for _ in 0..input.len() {
        if let Some(event) = reader.poll(now).unwrap() {
            let done = matches!(event, ResponseEvent::Complete);
            events.push(event);
            if done {
                break;
            }
        }
    }
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0], ResponseEvent::Echo));
    assert!(matches!(
        &events[1],
        ResponseEvent::Data(Reply::SimReadiness(SimStatus::Ready))
    ));
    assert!(
        matches!(&events[2], ResponseEvent::Notification(Notification::Unknown(s)) if s == "+CMTI: 1")
    );
    assert!(matches!(events[3], ResponseEvent::Complete));
    assert!(matches!(reader.poll(now), Err(ReplyReadError::Closed)));
    let mut transport = reader.finish().ok().unwrap();
    let mut remaining = String::new();
    transport.read_to_string(&mut remaining).unwrap();
    assert_eq!(remaining, "+CMTI: 2\r\n");
}

struct Unavailable(io::ErrorKind);
impl Read for Unavailable {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(self.0.into())
    }
}
#[test]
fn silence_expires_and_permanent_errors_stop_reading() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(1);
    for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted] {
        let mut reader =
            AtReplyReader::new(Unavailable(kind), Command::QuerySimReadiness, deadline);
        assert!(reader.poll(now).unwrap().is_none());
        assert!(matches!(
            reader.poll(deadline),
            Err(ReplyReadError::Response(ResponseError::TimedOut))
        ));
        assert!(matches!(reader.poll(deadline), Err(ReplyReadError::Closed)));
        assert!(reader.finish().is_err());
    }
    let mut reader = AtReplyReader::new(
        Unavailable(io::ErrorKind::BrokenPipe),
        Command::CheckConnection,
        deadline,
    );
    assert!(matches!(reader.poll(now), Err(ReplyReadError::Io(_))));
    assert!(matches!(reader.poll(now), Err(ReplyReadError::Closed)));
    assert!(reader.finish().is_err());
}

#[test]
fn malformed_input_rejection_and_eof_prevent_handoff() {
    let now = Instant::now();
    for input in [b"\0".as_slice(), b"ERROR\r\n", b""] {
        let mut reader = AtReplyReader::new(
            Cursor::new(input),
            Command::CheckConnection,
            now + Duration::from_secs(1),
        );
        let error = (0..=input.len())
            .find_map(|_| reader.poll(now).err())
            .unwrap();
        match input {
            b"\0" => assert!(matches!(error, ReplyReadError::Decode(_))),
            b"" => assert!(
                matches!(error, ReplyReadError::Io(e) if e.kind() == io::ErrorKind::UnexpectedEof)
            ),
            _ => assert!(matches!(
                error,
                ReplyReadError::Response(ResponseError::Rejected)
            )),
        }
        assert!(matches!(reader.poll(now), Err(ReplyReadError::Closed)));
        assert!(reader.finish().is_err());
    }
}
