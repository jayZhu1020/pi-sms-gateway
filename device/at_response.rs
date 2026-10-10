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

/// Keeps track of the modem's reply to one command.
///
/// For example, when asking `AT+CPIN?`, tell it to expect lines starting with
/// `+CPIN:`. It returns those lines as reply data, ignores the command's echo,
/// and passes other lines back as modem notifications. `OK` finishes the
/// command; an error or an expired deadline stops it.
///
/// Once stopped, this object cannot accept more lines. After a timeout, the
/// caller must recover the serial connection before sending another command,
/// so a delayed `OK` from the old command is not mistaken for the new reply.
/// This object tracks replies; it does not read or reset the serial port.
///
/// This simple matcher handles diagnostic replies. SMS payloads spanning
/// multiple lines, input prompts, and notifications that start like a reply
/// need additional parsing. Returned lines may contain private information.
pub struct AtResponse {
    command: &'static str,
    prefix: &'static str,
    deadline: Instant,
    closed: bool,
}

impl AtResponse {
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

    /// Call even when no bytes arrive, so silence still expires the command.
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

    /// Pass a complete line from AtLineDecoder; time is supplied for repeatable tests.
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
    use at_decoder::AtLineDecoder;
    use std::time::Duration;

    #[test]
    fn fragmented_exchange_keeps_notifications_separate() {
        let now = Instant::now();
        let mut response =
            AtResponse::new("AT+CPIN?", "+CPIN:", now + Duration::from_secs(4)).unwrap();
        let mut decoder = AtLineDecoder::default();
        let mut events = Vec::new();
        for chunk in [
            b"AT+CPIN?\r\n+CP".as_slice(),
            b"IN: READY\r\n+CMTI: \"SM\",7\r\nOK\r\n",
        ] {
            for &byte in chunk {
                if let Some(line) = decoder.push(byte).unwrap() {
                    events.push(response.accept(line, now).unwrap());
                }
            }
        }
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
