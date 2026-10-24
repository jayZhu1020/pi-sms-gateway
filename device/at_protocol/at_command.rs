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
    /// Inspect SMS representation without changing it.
    QuerySmsFormat,
    /// Inspect selected SMS memories and their occupancy without selecting a store.
    QuerySmsStorage,
}

impl Command {
    /// The writer adds the line ending; callers never build AT strings themselves.
    pub fn encode(self) -> &'static str {
        match self {
            Self::CheckConnection => "AT",
            Self::QuerySimReadiness => "AT+CPIN?",
            Self::QueryCircuitSwitchedRegistration => "AT+CREG?",
            Self::QueryLteRegistration => "AT+CEREG?",
            Self::QuerySmsFormat => "AT+CMGF?",
            Self::QuerySmsStorage => "AT+CPMS?",
        }
    }
    fn reply_prefix(self) -> Option<&'static str> {
        match self {
            Self::CheckConnection => None,
            Self::QuerySimReadiness => Some("+CPIN:"),
            Self::QueryCircuitSwitchedRegistration => Some("+CREG:"),
            Self::QueryLteRegistration => Some("+CEREG:"),
            Self::QuerySmsFormat => Some("+CMGF:"),
            Self::QuerySmsStorage => Some("+CPMS:"),
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
                Self::QuerySmsFormat => Reply::SmsFormat(match line[prefix.len()..].trim() {
                    "0" => SmsFormat::Pdu,
                    "1" => SmsFormat::Text,
                    _ => return Err(ParseError::InvalidReply),
                }),
                Self::QuerySmsStorage => Reply::SmsStorage(sms_storage(&line[prefix.len()..])?),
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

#[derive(PartialEq, Eq)]
pub enum Reply {
    SimReadiness(SimStatus),
    Registration(RegistrationStatus),
    SmsFormat(SmsFormat),
    SmsStorage(SmsStorage),
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

impl MessageStorage {
    fn from_wire(quoted: &str) -> Option<Self> {
        let name = quoted.strip_prefix('"')?.strip_suffix('"')?;
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return None;
        }
        Some(match name {
            "SM" => Self::Sim,
            "ME" => Self::Modem,
            "MT" => Self::Combined,
            other => Self::Other(other.to_owned()),
        })
    }
}
// Unknown codes remain private; presentation never includes modem-supplied text.
impl std::fmt::Display for MessageStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Sim => "SIM",
            Self::Modem => "modem",
            Self::Combined => "combined",
            Self::Other(_) => "other (unsupported)",
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SmsFormat {
    Pdu,
    Text,
}
impl std::fmt::Display for SmsFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pdu => "PDU",
            Self::Text => "text",
        })
    }
}

#[derive(PartialEq, Eq)]
pub struct StorageUsage {
    pub storage: MessageStorage,
    pub used: u32,
    pub total: u32,
}

/// Three roles may refer to the same physical store; do not sum their counts.
/// Receive storage is a preference, not proof that all messages are stored there.
#[derive(PartialEq, Eq)]
pub struct SmsStorage {
    pub read_delete: StorageUsage,
    pub write_send: StorageUsage,
    pub receive: StorageUsage,
}
fn sms_storage(payload: &str) -> Result<SmsStorage, ParseError> {
    let invalid = || ParseError::InvalidReply;
    let fields: Vec<_> = payload.split(',').map(str::trim).collect();
    if fields.len() != 9 {
        return Err(invalid());
    }
    let count = |value: &str| {
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        value.parse::<u32>().map_err(|_| invalid())
    };
    let usage = |fields: &[&str]| {
        let storage = MessageStorage::from_wire(fields[0]).ok_or_else(invalid)?;
        let used = count(fields[1])?;
        let total = count(fields[2])?;
        if used > total {
            return Err(invalid());
        }
        Ok(StorageUsage {
            storage,
            used,
            total,
        })
    };
    Ok(SmsStorage {
        read_delete: usage(&fields[..3])?,
        write_send: usage(&fields[3..6])?,
        receive: usage(&fields[6..])?,
    })
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
            let storage = MessageStorage::from_wire(memory.trim())?;
            let index = index.trim().parse().ok()?;
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
            (Command::QuerySmsFormat, "AT+CMGF?"),
            (Command::QuerySmsStorage, "AT+CPMS?"),
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
    fn decodes_sms_format_and_each_storage_role() {
        for (wire, expected) in [("+CMGF: 0", SmsFormat::Pdu), ("+CMGF: 1", SmsFormat::Text)] {
            assert!(
                matches!(Command::QuerySmsFormat.decode(wire.into()).unwrap(),
                DecodedLine::Reply(Reply::SmsFormat(format)) if format == expected)
            );
        }
        let line = Command::QuerySmsStorage
            .decode("+CPMS: \"SM\", 2, 20, \"ME\", 9, 9, \"MT\", 0, 0".into())
            .unwrap();
        let DecodedLine::Reply(Reply::SmsStorage(storage)) = line else {
            panic!("expected storage");
        };
        assert!(matches!(storage.read_delete.storage, MessageStorage::Sim));
        assert_eq!(
            (storage.read_delete.used, storage.read_delete.total),
            (2, 20)
        );
        assert!(matches!(storage.write_send.storage, MessageStorage::Modem));
        assert_eq!((storage.write_send.used, storage.write_send.total), (9, 9));
        assert!(matches!(storage.receive.storage, MessageStorage::Combined));
        assert_eq!((storage.receive.used, storage.receive.total), (0, 0));
    }

    #[test]
    fn preserves_unknown_storage_without_displaying_raw_code() {
        let line = Command::QuerySmsStorage
            .decode("+CPMS: \"FUTURE\",0,4294967295,\"SM\",0,1,\"ME\",0,1".into())
            .unwrap();
        let DecodedLine::Reply(Reply::SmsStorage(storage)) = line else {
            panic!("expected storage");
        };
        assert_eq!(
            storage.read_delete.storage.to_string(),
            "other (unsupported)"
        );
        assert!(
            matches!(storage.read_delete.storage, MessageStorage::Other(raw) if raw == "FUTURE")
        );
        assert_eq!(SmsFormat::Pdu.to_string(), "PDU");
        assert_eq!(SmsFormat::Text.to_string(), "text");
    }

    #[test]
    fn rejects_malformed_format_and_storage_with_sanitized_errors() {
        for wire in ["+CMGF: 2", "+CMGF:", "+CMGF: 0,1", "+CMGF: PRIVATE"] {
            assert!(matches!(
                Command::QuerySmsFormat.decode(wire.into()),
                Err(ParseError::InvalidReply)
            ));
        }
        for payload in [
            "",
            "0,1,0,1,0,1",           // write-command reply is not a query reply
            "\"SM\",0,1,\"ME\",0,1", // missing role
            "\"SM\",0,1,\"ME\",0,1,\"SM\",0,1,extra",
            "SM,0,1,\"ME\",0,1,\"SM\",0,1", // missing quotes
            "\"\",0,1,\"ME\",0,1,\"SM\",0,1",
            "\"PRIVATE!\",0,1,\"ME\",0,1,\"SM\",0,1",
            "\"SM\",2,1,\"ME\",0,1,\"SM\",0,1", // impossible occupancy
            "\"SM\",-1,1,\"ME\",0,1,\"SM\",0,1",
            "\"SM\",+1,1,\"ME\",0,1,\"SM\",0,1",
            "\"SM\",0,4294967296,\"ME\",0,1,\"SM\",0,1",
            "\"SM\",0,1,\"ME\",,1,\"SM\",0,1",
            "\"SM\",0,1,\"ME\",0,1,\"SM\",2,1",
        ] {
            let wire = format!("+CPMS: {payload}");
            assert!(matches!(
                Command::QuerySmsStorage.decode(wire).err(),
                Some(ParseError::InvalidReply)
            ));
        }
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
