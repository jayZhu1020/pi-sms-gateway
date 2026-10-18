//! One-shot Linux CLI for read-only SIM and network registration diagnostics.
use serial_port::SerialPort;
use sim7600_probe::{SimStatus, probe_network, probe_sim};
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!(
            "Usage: sim_probe DEVICE\nQuery SIM and network registration on a synchronized SIM7600 AT port (5s per stage)."
        );
        return Ok(());
    }
    if args.len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: sim_probe DEVICE",
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
