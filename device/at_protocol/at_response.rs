//! Track one diagnostic AT command after its bytes have been sent.
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum ResponseError {
    InvalidPrefix,
    Rejected,
    TimedOut,
    Closed,
}

pub enum ResponseEvent {
    Echo,
    Data(String),
    Notification(String),
    Complete,
}

/// Tracks the modem's reply to one diagnostic command.
///
/// `new(command, prefix, deadline)` takes two static strings and an `Instant`.
/// For `AT+CPIN?`, use `+CPIN:` as the reply prefix. It returns a tracker,
/// or `InvalidPrefix` if the prefix is empty; it does not send the command.
///
/// `accept(line, now)` takes one complete `String` and the current time.
/// It returns `Echo`, `Data(line)`, `Notification(line)`, or `Complete` on `OK`.
/// Modem errors return `Rejected`. Other lines are treated as notifications;
/// matching the prefix does not validate the reply's contents.
///
/// `check_deadline(now)` takes the current time and returns `Ok(())` while
/// active, `TimedOut` at/after the deadline, or `Closed` once stopped.
/// Call it while idle too. Completion, rejection, or timeout stops the tracker.
/// After timeout, recover the port before a new command so an old `OK` cannot
/// look like its reply. This object does not read or reset the port.
/// Multiline payloads, prompts, and overlapping reply/notification prefixes
/// need additional parsing. Returned lines may contain private data.
pub struct AtResponse {
    command: &'static str,
    prefix: &'static str,
    deadline: Instant,
    closed: bool,
}

impl AtResponse {
    /// Create a tracker from the sent command, expected reply prefix, and deadline.
    pub fn new(
        command: &'static str,
        prefix: &'static str,
        deadline: Instant,
    ) -> Result<Self, ResponseError> {
        if prefix.is_empty() {
            return Err(ResponseError::InvalidPrefix);
        }
        Ok(Self {
            command,
            prefix,
            deadline,
            closed: false,
        })
    }

    /// Return whether the command is active; expire it even when no line arrives.
    pub fn check_deadline(&mut self, now: Instant) -> Result<(), ResponseError> {
        if self.closed {
            return Err(ResponseError::Closed);
        }
        if now >= self.deadline {
            self.closed = true;
            return Err(ResponseError::TimedOut);
        }
        Ok(())
    }

    /// Classify a complete line at the given time; return an event or command error.
    pub fn accept(&mut self, line: String, now: Instant) -> Result<ResponseEvent, ResponseError> {
        self.check_deadline(now)?;
        if line == self.command {
            return Ok(ResponseEvent::Echo);
        }
        if line == "OK" {
            self.closed = true;
            return Ok(ResponseEvent::Complete);
        }
        if line == "ERROR" || line.starts_with("+CME ERROR:") || line.starts_with("+CMS ERROR:") {
            self.closed = true;
            return Err(ResponseError::Rejected);
        }
        if line.starts_with(self.prefix) {
            Ok(ResponseEvent::Data(line))
        } else {
            Ok(ResponseEvent::Notification(line))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn complete_lines_keep_notifications_separate() {
        let now = Instant::now();
        let mut response =
            AtResponse::new("AT+CPIN?", "+CPIN:", now + Duration::from_secs(4)).unwrap();
        let lines = ["AT+CPIN?", "+CPIN: READY", "+CMTI: \"SM\",7", "OK"];
        let events: Vec<_> = lines
            .into_iter()
            .map(|line| response.accept(line.into(), now).unwrap())
            .collect();
        assert!(matches!(events[0], ResponseEvent::Echo));
        assert!(matches!(&events[1], ResponseEvent::Data(s) if s == "+CPIN: READY"));
        assert!(matches!(&events[2], ResponseEvent::Notification(s) if s == "+CMTI: \"SM\",7"));
        assert!(matches!(events[3], ResponseEvent::Complete));
        assert_eq!(events.len(), 4);
        assert!(matches!(
            response.accept("OK".into(), now),
            Err(ResponseError::Closed)
        ));
    }

    #[test]
    fn modem_errors_close_the_command() {
        let now = Instant::now();
        for error in ["ERROR", "+CME ERROR: 10", "+CMS ERROR: 500"] {
            let mut response =
                AtResponse::new("AT+CPIN?", "+CPIN:", now + Duration::from_secs(4)).unwrap();
            assert!(matches!(
                response.accept(error.into(), now),
                Err(ResponseError::Rejected)
            ));
            assert_eq!(response.check_deadline(now), Err(ResponseError::Closed));
        }
    }

    #[test]
    fn silence_and_late_ok_cannot_complete_an_expired_command() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(4);
        let mut response = AtResponse::new("AT+CPIN?", "+CPIN:", deadline).unwrap();
        assert_eq!(
            response.check_deadline(deadline),
            Err(ResponseError::TimedOut)
        );
        assert!(matches!(
            response.accept("OK".into(), deadline),
            Err(ResponseError::Closed)
        ));
        let mut response = AtResponse::new("AT+CPIN?", "+CPIN:", deadline).unwrap();
        assert!(matches!(
            response.accept("OK".into(), deadline),
            Err(ResponseError::TimedOut)
        ));
        assert!(matches!(
            AtResponse::new("AT", "", deadline),
            Err(ResponseError::InvalidPrefix)
        ));
    }
}
