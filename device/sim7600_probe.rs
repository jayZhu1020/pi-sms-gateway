//! Read-only SIM7600 diagnostics expressed through semantic AT commands.
use at_command::{Command, Reply};
pub use at_command::{RegistrationStatus, SimStatus};
use at_exchange::AtExchange;
use at_response::ResponseEvent;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct SimProbe {
    pub status: SimStatus,
    pub notifications: u32,
}

/// Query SIM PIN status on an already-synchronized, nonblocking connection.
/// Returns the transport and typed status only after a valid SIM-readiness reply and completion.
/// Notifications are counted, not logged; this one-shot probe does not store them.
/// Errors drop the transport. No SIM unlocking, retries or recovery are performed.
/// The deadline covers writing/reading, not opening/restoring terminal settings.
pub fn probe_sim<T: Read + Write>(transport: T, deadline: Instant) -> io::Result<(T, SimProbe)> {
    let (transport, reply, notifications) = query(transport, Command::QuerySimReadiness, deadline)?;
    let Reply::SimReadiness(status) = reply else {
        return Err(io::Error::other("unexpected SIM reply type"));
    };
    Ok((
        transport,
        SimProbe {
            status,
            notifications,
        },
    ))
}

#[derive(Debug)]
pub struct NetworkProbe {
    pub circuit_switched: RegistrationStatus,
    pub eps: RegistrationStatus,
    pub notifications: u32,
}

/// Query circuit-switched and LTE registration sequentially on the same synchronized transport.
/// The supplied deadline covers both exchanges. No registration modes change.
/// Location fields are validated then discarded; errors never include raw replies.
/// Conflicting/ambiguous same-prefix notifications fail rather than guess a status.
pub fn probe_network<T: Read + Write>(
    transport: T,
    deadline: Instant,
) -> io::Result<(T, NetworkProbe)> {
    let (transport, reply, first_notifications) = query(
        transport,
        Command::QueryCircuitSwitchedRegistration,
        deadline,
    )?;
    let Reply::Registration(circuit_switched) = reply else {
        return Err(io::Error::other("unexpected registration reply type"));
    };
    let (transport, reply, notifications) =
        query(transport, Command::QueryLteRegistration, deadline)?;
    let Reply::Registration(eps) = reply else {
        return Err(io::Error::other("unexpected registration reply type"));
    };
    Ok((
        transport,
        NetworkProbe {
            circuit_switched,
            eps,
            notifications: first_notifications.saturating_add(notifications),
        },
    ))
}

// Require exactly one typed diagnostic reply before successful completion.
fn query<T: Read + Write>(
    transport: T,
    command: Command,
    deadline: Instant,
) -> io::Result<(T, Reply, u32)> {
    let mut exchange = AtExchange::new(transport, command, deadline);
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
