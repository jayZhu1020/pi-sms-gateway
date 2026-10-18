//! Send one ASCII AT command through a nonblocking byte transport.
use at_command::Command;
use std::io::{self, Write};
use std::time::Instant;

/// Sends a command once, including its final CR byte.
/// `new(transport, command, deadline)` takes ownership of a nonblocking writer.
/// Only supported semantic commands can be sent; the translation layer owns
/// their printable wire encoding. Arbitrary caller-supplied strings are not accepted.
/// `poll(now)` attempts one write: true means all bytes were accepted, false means
/// retry later. Partial writes resume at the next byte; Interrupted/WouldBlock retry.
/// Poll while idle to enforce the deadline. Other errors permanently stop writing.
/// `finish()` returns the writer only after success; otherwise returns self.
/// Success means bytes accepted by the OS, not modem acknowledgment or UART drain.
/// After failure, drop this owner; a later session must recover before another command.
/// No reply parsing, sleep, blocking-IO protection, or automatic recovery is provided.
pub struct AtCommandWriter<W> {
    transport: W,
    bytes: Vec<u8>,
    offset: usize,
    deadline: Instant,
    failed: bool,
}
impl<W: Write> AtCommandWriter<W> {
    pub fn new(transport: W, command: Command, deadline: Instant) -> Self {
        let mut bytes = command.encode().as_bytes().to_vec();
        bytes.push(at_constant::CR);
        Self {
            transport,
            bytes,
            offset: 0,
            deadline,
            failed: false,
        }
    }
    pub fn poll(&mut self, now: Instant) -> io::Result<bool> {
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "command writer stopped",
            ));
        }
        if self.offset == self.bytes.len() {
            return Ok(true);
        }
        self.failed = true;
        if now >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "command write expired",
            ));
        }
        match self.transport.write(&self.bytes[self.offset..]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => self.offset += count,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
        self.failed = false;
        Ok(self.offset == self.bytes.len())
    }
    /// Return the transport if all command bytes were written.
    /// Otherwise, return this command writer without writing more bytes.
    pub fn finish(self) -> Result<W, Self> {
        if !self.failed && self.offset == self.bytes.len() {
            Ok(self.transport)
        } else {
            Err(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Duration;

    struct ScriptedWriter {
        outcomes: VecDeque<io::Result<usize>>,
        accepted: Vec<u8>,
    }
    impl Write for ScriptedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = self
                .outcomes
                .pop_front()
                .expect("unexpected extra write")?
                .min(bytes.len());
            self.accepted.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!("flush is not UART drain")
        }
    }
    fn scripted(outcomes: Vec<io::Result<usize>>) -> ScriptedWriter {
        ScriptedWriter {
            outcomes: outcomes.into(),
            accepted: Vec::new(),
        }
    }
    #[test]
    fn resumes_short_writes_without_repeating_bytes() {
        let now = Instant::now();
        let transport = scripted(vec![
            Ok(2),
            Err(io::ErrorKind::WouldBlock.into()),
            Err(io::ErrorKind::Interrupted.into()),
            Ok(99),
        ]);
        let mut writer = AtCommandWriter::new(
            transport,
            Command::QuerySimReadiness,
            now + Duration::from_secs(1),
        );
        for expected in [false, false, false, true] {
            assert_eq!(writer.poll(now).unwrap(), expected);
        }
        assert!(writer.poll(now + Duration::from_secs(2)).unwrap());
        let transport = writer.finish().ok().unwrap();
        assert_eq!(transport.accepted, b"AT+CPIN?\r");
        assert!(transport.outcomes.is_empty());
    }
    #[test]
    fn failure_or_deadline_prevents_further_writes_and_handoff() {
        let now = Instant::now();
        for error in [Ok(0), Err(io::ErrorKind::PermissionDenied.into())] {
            let mut writer = AtCommandWriter::new(
                scripted(vec![Ok(1), error]),
                Command::CheckConnection,
                now + Duration::from_secs(1),
            );
            assert!(!writer.poll(now).unwrap());
            assert!(writer.poll(now).is_err());
            assert_eq!(
                writer.poll(now).unwrap_err().kind(),
                io::ErrorKind::BrokenPipe
            );
            assert!(writer.finish().is_err());
        }
        let mut writer = AtCommandWriter::new(
            scripted(vec![Ok(1)]),
            Command::CheckConnection,
            now + Duration::from_secs(1),
        );
        assert!(!writer.poll(now).unwrap());
        assert_eq!(
            writer
                .poll(now + Duration::from_secs(1))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(writer.finish().is_err());
    }
    #[test]
    fn unfinished_command_cannot_be_handed_off() {
        let writer =
            AtCommandWriter::new(scripted(vec![]), Command::CheckConnection, Instant::now());
        assert!(writer.finish().is_err());
    }
}
