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
    let mut exchange = AtExchange::new(transport, "AT+CPIN?", "+CPIN:", deadline)?;
    let mut status = None;
    let mut notifications: u32 = 0;
    loop {
        match exchange
            .poll(Instant::now())
            .map_err(|e| io::Error::other(format!("{e:?}")))?
        {
            Some(ResponseEvent::Data(line)) => {
                if status.is_some() {
                    return Err(io::Error::other("duplicate SIM status"));
                }
                status = Some(match line.trim() {
                    "+CPIN: READY" => SimStatus::Ready,
                    "+CPIN: SIM PIN" => SimStatus::PinRequired,
                    "+CPIN: SIM PUK" => SimStatus::PukRequired,
                    _ => return Err(io::Error::other("unsupported SIM status")),
                });
            }
            Some(ResponseEvent::Notification(_)) => notifications = notifications.saturating_add(1),
            Some(ResponseEvent::Complete) => {
                let status = status.ok_or_else(|| io::Error::other("reply omitted SIM status"))?;
                let transport = exchange
                    .finish()
                    .map_err(|_| io::Error::other("incomplete exchange"))?;
                return Ok((
                    transport,
                    SimProbe {
                        status,
                        notifications,
                    },
                ));
            }
            Some(ResponseEvent::Echo) => {}
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    }
}
