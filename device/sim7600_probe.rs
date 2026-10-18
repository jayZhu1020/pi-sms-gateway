//! Read-only SIM7600 diagnostics; modem-specific commands stay in this module.
use at_exchange::AtExchange;
use at_response::ResponseEvent;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub enum SimStatus {
    Ready,
    PinRequired,
    PukRequired,
}
#[derive(Debug)]
pub struct SimProbe {
    pub status: SimStatus,
    pub notifications: u32,
}

/// Query SIM PIN status on an already-synchronized, nonblocking connection.
/// Returns the transport and typed status only after one valid CPIN line and OK.
/// Notifications are counted, not logged; this one-shot probe does not store them.
/// Errors drop the transport. No SIM unlocking, retries or recovery are performed.
/// The deadline covers writing/reading, not opening/restoring terminal settings.
pub fn probe_sim<T: Read + Write>(transport: T, deadline: Instant) -> io::Result<(T, SimProbe)> {
    let (transport, line, notifications) = query(transport, "AT+CPIN?", "+CPIN:", deadline)?;
    let status = match line.trim() {
        "+CPIN: READY" => SimStatus::Ready,
        "+CPIN: SIM PIN" => SimStatus::PinRequired,
        "+CPIN: SIM PUK" => SimStatus::PukRequired,
        _ => return Err(io::Error::other("unsupported SIM status")),
    };
    Ok((
        transport,
        SimProbe {
            status,
            notifications,
        },
    ))
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
#[derive(Debug)]
pub struct NetworkProbe {
    pub circuit_switched: RegistrationStatus,
    pub eps: RegistrationStatus,
    pub notifications: u32,
}

/// Read CREG and CEREG sequentially on the same synchronized transport.
/// The supplied deadline covers both exchanges. No registration modes change.
/// Location fields are validated then discarded; errors never include raw replies.
/// Conflicting/ambiguous same-prefix notifications fail rather than guess a status.
pub fn probe_network<T: Read + Write>(
    transport: T,
    deadline: Instant,
) -> io::Result<(T, NetworkProbe)> {
    let (transport, line, first_notifications) = query(transport, "AT+CREG?", "+CREG:", deadline)?;
    let circuit_switched = registration(&line, "+CREG:", false)?;
    let (transport, line, notifications) = query(transport, "AT+CEREG?", "+CEREG:", deadline)?;
    let eps = registration(&line, "+CEREG:", true)?;
    Ok((
        transport,
        NetworkProbe {
            circuit_switched,
            eps,
            notifications: first_notifications.saturating_add(notifications),
        },
    ))
}

fn registration(line: &str, prefix: &str, eps: bool) -> io::Result<RegistrationStatus> {
    let invalid = || io::Error::other("invalid registration reply");
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

// One diagnostic data line plus final OK. Keep modem-specific framing in this adapter.
fn query<T: Read + Write>(
    transport: T,
    command: &'static str,
    prefix: &'static str,
    deadline: Instant,
) -> io::Result<(T, String, u32)> {
    let mut exchange = AtExchange::new(transport, command, prefix, deadline)?;
    let mut data = None;
    let mut notifications: u32 = 0;
    loop {
        match exchange
            .poll(Instant::now())
            .map_err(|e| io::Error::other(format!("{e:?}")))?
        {
            Some(ResponseEvent::Data(line)) => {
                if data.replace(line).is_some() {
                    return Err(io::Error::other("duplicate diagnostic reply"));
                }
            }
            Some(ResponseEvent::Notification(_)) => notifications = notifications.saturating_add(1),
            Some(ResponseEvent::Complete) => {
                let data = data.ok_or_else(|| io::Error::other("reply omitted diagnostic data"))?;
                let transport = exchange
                    .finish()
                    .map_err(|_| io::Error::other("incomplete exchange"))?;
                return Ok((transport, data, notifications));
            }
            Some(ResponseEvent::Echo) => {}
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    }
}
