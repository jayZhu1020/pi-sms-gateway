//! Translate supported modem wire codes into meaningful commands and events.
//! Add wire spellings and payload decoding here; consumers use the semantic types.

/// Supported operations are read-only. The type binds each request to its reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Check that the modem can respond.
    CheckConnection,
    /// Query whether the SIM is ready or needs a PIN/PUK.
    QuerySimReadiness,
    /// Query registration for circuit-switched services.
    QueryCircuitSwitchedRegistration,
    /// Query LTE/EPS registration.
    QueryLteRegistration,
}

impl Command {
    /// The writer adds the line ending; callers never build AT strings themselves.
    pub fn encode(self) -> &'static str {
        match self {
            Self::CheckConnection => "AT",
            Self::QuerySimReadiness => "AT+CPIN?",
            Self::QueryCircuitSwitchedRegistration => "AT+CREG?",
            Self::QueryLteRegistration => "AT+CEREG?",
        }
    }
    fn reply_prefix(self) -> Option<&'static str> {
        match self {
            Self::CheckConnection => None,
            Self::QuerySimReadiness => Some("+CPIN:"),
            Self::QueryCircuitSwitchedRegistration => Some("+CREG:"),
            Self::QueryLteRegistration => Some("+CEREG:"),
        }
    }
    /// Translate one framed line in the context of the command that was sent.
    /// Unknown/malformed notifications retain their bytes without implementing Debug.
    /// Invalid expected replies fail with a sanitized error; no location data escapes.
    pub fn decode(self, line: String) -> Result<DecodedLine, ParseError> {
        if line == self.encode() {
            return Ok(DecodedLine::Echo);
        }
        if line == "OK" {
            return Ok(DecodedLine::Complete);
        }
        if line == "ERROR" || line.starts_with("+CME ERROR:") || line.starts_with("+CMS ERROR:") {
            return Ok(DecodedLine::Rejected);
        }
        if let Some(prefix) = self
            .reply_prefix()
            .filter(|prefix| line.starts_with(prefix))
        {
            let reply = match self {
                Self::QuerySimReadiness => Reply::SimReadiness(match line[prefix.len()..].trim() {
                    "READY" => SimStatus::Ready,
                    "SIM PIN" => SimStatus::PinRequired,
                    "SIM PUK" => SimStatus::PukRequired,
                    _ => return Err(ParseError::InvalidReply),
                }),
                Self::QueryCircuitSwitchedRegistration | Self::QueryLteRegistration => {
                    Reply::Registration(registration(
                        &line,
                        prefix,
                        self == Self::QueryLteRegistration,
                    )?)
                }
                Self::CheckConnection => unreachable!("attention command has no data prefix"),
            };
            return Ok(DecodedLine::Reply(reply));
        }
        Ok(DecodedLine::Notification(Notification::decode(line)))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    InvalidReply,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reply {
    SimReadiness(SimStatus),
    Registration(RegistrationStatus),
}

pub enum DecodedLine {
    Echo,
    Complete,
    Rejected,
    Reply(Reply),
    Notification(Notification),
}

/// Storage identifiers translated from modem memory codes. Unknown values are private.
#[derive(PartialEq, Eq)]
pub enum MessageStorage {
    Sim,
    Modem,
    Combined,
    Other(String),
}

/// A stored-SMS indication identifies a slot, not a durable message identity.
/// Raw unknown notifications may contain private data and must not be logged.
#[derive(PartialEq, Eq)]
pub enum Notification {
    SmsStored { storage: MessageStorage, index: u32 },
    Unknown(String),
}
impl Notification {
    fn decode(line: String) -> Self {
        let stored = (|| {
            let (memory, index) = line.strip_prefix("+CMTI:")?.split_once(',')?;
            let memory = memory.trim().strip_prefix('"')?.strip_suffix('"')?;
            if memory.is_empty() || !memory.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return None;
            }
            let index = index.trim().parse().ok()?;
            let storage = match memory {
                "SM" => MessageStorage::Sim,
                "ME" => MessageStorage::Modem,
                "MT" => MessageStorage::Combined,
                other => MessageStorage::Other(other.to_owned()),
            };
            Some(Self::SmsStored { storage, index })
        })();
        stored.unwrap_or(Self::Unknown(line))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SimStatus {
    Ready,
    PinRequired,
    PukRequired,
}

// Preserve the diagnostic CLI's established status labels at the translation boundary.
impl std::fmt::Display for SimStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Ready => "READY",
            Self::PinRequired => "SIM PIN",
            Self::PukRequired => "SIM PUK",
        })
    }
}

/// Snapshot of the modem's reported registration, not proof of SMS delivery.
#[derive(Debug, PartialEq, Eq)]
pub enum RegistrationStatus {
    NotRegistered,
    Home,
    Searching,
    Denied,
    Unknown,
    Roaming,
    SmsOnlyHome,
    SmsOnlyRoaming,
    EmergencyOnly,
}
impl std::fmt::Display for RegistrationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotRegistered => "not registered",
            Self::Home => "registered (home)",
            Self::Searching => "searching",
            Self::Denied => "registration denied",
            Self::Unknown => "unknown",
            Self::Roaming => "registered (roaming)",
            Self::SmsOnlyHome => "registered (SMS only, home)",
            Self::SmsOnlyRoaming => "registered (SMS only, roaming)",
            Self::EmergencyOnly => "emergency services only",
        })
    }
}

fn registration(line: &str, prefix: &str, eps: bool) -> Result<RegistrationStatus, ParseError> {
    let invalid = || ParseError::InvalidReply;
    let fields: Vec<_> = line
        .strip_prefix(prefix)
        .ok_or_else(invalid)?
        .split(',')
        .map(str::trim)
        .collect();
    if !matches!(fields.len(), 2 | 4 | 5) {
        return Err(invalid());
    }
    let mode: u8 = fields[0].parse().map_err(|_| invalid())?;
    if mode > 2 || (fields.len() > 2 && mode != 2) {
        return Err(invalid());
    }
    if fields.len() > 2 {
        for (field, max) in [(fields[2], 4), (fields[3], 8)] {
            let hex = field
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .ok_or_else(invalid)?;
            if hex.len() > max || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid());
            }
        }
        if fields.len() == 5 && !matches!(fields[4].parse::<u8>(), Ok(0..=7)) {
            return Err(invalid());
        }
    }
    match fields[1].parse::<u8>() {
        Ok(0) => Ok(RegistrationStatus::NotRegistered),
        Ok(1) => Ok(RegistrationStatus::Home),
        Ok(2) => Ok(RegistrationStatus::Searching),
        Ok(3) => Ok(RegistrationStatus::Denied),
        Ok(4) => Ok(RegistrationStatus::Unknown),
        Ok(5) => Ok(RegistrationStatus::Roaming),
        Ok(6) if eps => Ok(RegistrationStatus::SmsOnlyHome),
        Ok(7) if eps => Ok(RegistrationStatus::SmsOnlyRoaming),
        Ok(8) if eps => Ok(RegistrationStatus::EmergencyOnly),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_encode_exact_independent_wire_fixtures() {
        for (command, expected) in [
            (Command::CheckConnection, "AT"),
            (Command::QuerySimReadiness, "AT+CPIN?"),
            (Command::QueryCircuitSwitchedRegistration, "AT+CREG?"),
            (Command::QueryLteRegistration, "AT+CEREG?"),
        ] {
            assert_eq!(command.encode(), expected);
            assert!(command.encode().bytes().all(|b| matches!(b, b' '..=b'~')));
        }
    }

    #[test]
    fn decodes_meaningful_replies_and_command_result_codes() {
        let command = Command::QuerySimReadiness;
        for (wire, status) in [
            ("+CPIN: READY", SimStatus::Ready),
            ("+CPIN: SIM PIN", SimStatus::PinRequired),
            ("+CPIN: SIM PUK", SimStatus::PukRequired),
        ] {
            assert!(matches!(command.decode(wire.into()).unwrap(),
                DecodedLine::Reply(Reply::SimReadiness(value)) if value == status));
        }
        assert!(matches!(
            command.decode("AT+CPIN?".into()).unwrap(),
            DecodedLine::Echo
        ));
        assert!(matches!(
            command.decode("OK".into()).unwrap(),
            DecodedLine::Complete
        ));
        for wire in ["ERROR", "+CME ERROR: 13", "+CMS ERROR: 500"] {
            assert!(matches!(
                command.decode(wire.into()).unwrap(),
                DecodedLine::Rejected
            ));
        }
        assert!(matches!(
            Command::QueryLteRegistration
                .decode("+CEREG: 0,5".into())
                .unwrap(),
            DecodedLine::Reply(Reply::Registration(RegistrationStatus::Roaming))
        ));
        assert!(matches!(
            Command::QueryCircuitSwitchedRegistration
                .decode("+CREG: 0,1".into())
                .unwrap(),
            DecodedLine::Reply(Reply::Registration(RegistrationStatus::Home))
        ));
    }

    #[test]
    fn translates_stored_sms_indications_without_losing_future_storage_codes() {
        for (code, storage) in [
            ("SM", MessageStorage::Sim),
            ("ME", MessageStorage::Modem),
            ("MT", MessageStorage::Combined),
            ("SR", MessageStorage::Other("SR".into())),
        ] {
            let wire = format!("+CMTI: \"{code}\",7");
            let line = Command::QuerySimReadiness.decode(wire).unwrap();
            assert!(
                matches!(line, DecodedLine::Notification(Notification::SmsStored {
                storage: actual, index: 7 }) if actual == storage)
            );
        }
    }

    #[test]
    fn preserves_unrecognized_and_malformed_notifications_verbatim() {
        for wire in [
            "+FUTURE: private",
            "+CMTI: 1",
            "+CMTI: \"SM\",nope",
            "+CMTI: \"SM\",4294967296",
            "+CMTI: \"SM\",1,2",
            "+CMTI: \"\",1",
        ] {
            assert!(
                matches!(Command::QuerySimReadiness.decode(wire.into()).unwrap(),
                DecodedLine::Notification(Notification::Unknown(raw)) if raw == wire)
            );
        }
        assert!(matches!(
            Command::QuerySimReadiness
                .decode("+CREG: 0,1".into())
                .unwrap(),
            DecodedLine::Notification(Notification::Unknown(_))
        ));
    }

    #[test]
    fn malformed_expected_replies_return_only_sanitized_errors() {
        for (command, wire) in [
            (Command::QuerySimReadiness, "+CPIN: PRIVATE"),
            (
                Command::QueryCircuitSwitchedRegistration,
                "+CREG: 2,1,\"SECRET\",\"1234\"",
            ),
            (Command::QueryLteRegistration, "+CEREG: 1"),
        ] {
            assert!(matches!(
                command.decode(wire.into()),
                Err(ParseError::InvalidReply)
            ));
        }
    }
}
