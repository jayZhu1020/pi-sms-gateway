//! One-shot Linux CLI for read-only SIM and network registration diagnostics.
use serial_port::SerialPort;
use sim7600_probe::{SimStatus, probe_network, probe_sim, probe_sms_storage};
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!(
            "Usage: sim_probe DEVICE [--sms-storage]\nQuery SIM/network status; optionally inspect SMS format and storage (5s per stage)."
        );
        return Ok(());
    }
    let inspect_storage = args.len() == 2 && args[1] == "--sms-storage";
    if args.len() != 1 && !inspect_storage {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: sim_probe DEVICE [--sms-storage]",
        ));
    }
    let port = SerialPort::open(Path::new(&args[0]))?;
    let (port, result) = probe_sim(port, Instant::now() + Duration::from_secs(5))?;
    let (port, network) = if result.status == SimStatus::Ready {
        let (port, network) = probe_network(port, Instant::now() + Duration::from_secs(5))?;
        (port, Some(network))
    } else {
        (port, None)
    };
    let (port, sms) = if inspect_storage && result.status == SimStatus::Ready {
        let (port, sms) = probe_sms_storage(port, Instant::now() + Duration::from_secs(5))?;
        (port, Some(sms))
    } else {
        (port, None)
    };
    port.close()?;
    println!("SIM status: {}", result.status);
    let mut notifications = result.notifications;
    if let Some(network) = network {
        println!(
            "Circuit-switched registration: {}",
            network.circuit_switched
        );
        println!("LTE/EPS registration: {}", network.eps);
        notifications = notifications.saturating_add(network.notifications);
    } else {
        println!("Network registration: skipped (SIM locked)");
    }
    if let Some(sms) = sms {
        println!("SMS format: {}", sms.format);
        for (role, usage) in [
            ("read/delete", sms.storage.read_delete),
            ("write/send", sms.storage.write_send),
            ("receive (preferred)", sms.storage.receive),
        ] {
            println!(
                "SMS storage {role}: {}, {} used / {} total",
                usage.storage, usage.used, usage.total
            );
        }
        notifications = notifications.saturating_add(sms.notifications);
    } else if inspect_storage {
        println!("SMS storage: skipped (SIM locked)");
    }
    println!("Notifications observed: {notifications}");
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sim_probe: {error}");
            ExitCode::FAILURE
        }
    }
}
