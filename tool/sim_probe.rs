//! One-shot Linux CLI for the SIM7600 read-only SIM readiness probe.
use serial_port::SerialPort;
use sim7600_probe::{SimStatus, probe_sim};
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--help" {
        println!(
            "Usage: sim_probe DEVICE\nQuery SIM status on a synchronized SIM7600 AT port (5s exchange deadline)."
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
    port.close()?;
    let status = match result.status {
        SimStatus::Ready => "READY",
        SimStatus::PinRequired => "SIM PIN",
        SimStatus::PukRequired => "SIM PUK",
    };
    println!(
        "SIM status: {status}\nNotifications observed: {}",
        result.notifications
    );
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
