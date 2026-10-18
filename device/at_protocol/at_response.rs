//! Track one semantic command after its bytes have been sent.
use at_command::{Command, DecodedLine, Notification, Reply};
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum ResponseError {
    InvalidReply,
    Rejected,
    TimedOut,
    Closed,
}

pub enum ResponseEvent {
    Echo,
    Data(Reply),
    Notification(Notification),
    Complete,
}

/// Track completion/deadlines while the semantic interface translates each line.
/// The command determines its encoding and expected reply; callers cannot pair
/// unrelated request strings and prefixes. Malformed expected replies and modem
/// errors stop the tracker. Unknown notifications are retained as private data.
/// After timeout, recover the connection before another command. Multiline
/// payloads, prompts and ambiguous same-prefix notifications need a session owner.
pub struct AtResponse {
    command: Command,
    deadline: Instant,
    closed: bool,
}
impl AtResponse {
    pub fn new(command: Command, deadline: Instant) -> Self {
        Self {
            command,
            deadline,
            closed: false,
        }
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
    pub fn accept(&mut self, line: String, now: Instant) -> Result<ResponseEvent, ResponseError> {
        self.check_deadline(now)?;
        let decoded = self.command.decode(line).map_err(|_| {
            self.closed = true;
            ResponseError::InvalidReply
        })?;
        match decoded {
            DecodedLine::Echo => Ok(ResponseEvent::Echo),
            DecodedLine::Reply(reply) => Ok(ResponseEvent::Data(reply)),
            DecodedLine::Notification(notification) => {
                Ok(ResponseEvent::Notification(notification))
            }
            DecodedLine::Complete => {
                self.closed = true;
                Ok(ResponseEvent::Complete)
            }
            DecodedLine::Rejected => {
                self.closed = true;
                Err(ResponseError::Rejected)
            }
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
            AtResponse::new(Command::QuerySimReadiness, now + Duration::from_secs(4));
        let lines = ["AT+CPIN?", "+CPIN: READY", "+CMTI: \"SM\",7", "OK"];
        let events: Vec<_> = lines
            .into_iter()
            .map(|line| response.accept(line.into(), now).unwrap())
            .collect();
        assert!(matches!(events[0], ResponseEvent::Echo));
        assert!(matches!(
            &events[1],
            ResponseEvent::Data(Reply::SimReadiness(at_command::SimStatus::Ready))
        ));
        assert!(matches!(
            &events[2],
            ResponseEvent::Notification(Notification::SmsStored {
                storage: at_command::MessageStorage::Sim,
                index: 7
            })
        ));
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
                AtResponse::new(Command::QuerySimReadiness, now + Duration::from_secs(4));
            assert!(matches!(
                response.accept(error.into(), now),
                Err(ResponseError::Rejected)
            ));
            assert_eq!(response.check_deadline(now), Err(ResponseError::Closed));
        }
    }

    #[test]
    fn invalid_expected_payload_closes_without_exposing_raw_data() {
        let now = Instant::now();
        let mut response =
            AtResponse::new(Command::QuerySimReadiness, now + Duration::from_secs(1));
        assert!(matches!(
            response.accept("+CPIN: PRIVATE".into(), now),
            Err(ResponseError::InvalidReply)
        ));
        assert!(matches!(
            response.accept("OK".into(), now),
            Err(ResponseError::Closed)
        ));
    }

    #[test]
    fn silence_and_late_ok_cannot_complete_an_expired_command() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(4);
        let mut response = AtResponse::new(Command::QuerySimReadiness, deadline);
        assert_eq!(
            response.check_deadline(deadline),
            Err(ResponseError::TimedOut)
        );
        assert!(matches!(
            response.accept("OK".into(), deadline),
            Err(ResponseError::Closed)
        ));
        let mut response = AtResponse::new(Command::QuerySimReadiness, deadline);
        assert!(matches!(
            response.accept("OK".into(), deadline),
            Err(ResponseError::TimedOut)
        ));
    }
}
