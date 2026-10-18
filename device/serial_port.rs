//! Own a serial device and restore its terminal settings when finished.
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use terminal_options::{DataBits, Parity, SavedSettings, StopBits, TerminalOptions};

// Open without waiting for carrier detection or taking control of this terminal.
const OPEN_FLAGS: i32 = 0x800 | 0x100; // Linux O_NONBLOCK | O_NOCTTY

/// Owns one serial port for raw byte reads and writes.
///
/// `open(path)` takes a device path and returns a locked, nonblocking port or
/// an IO error. It preserves baud and configures 8 data bits, no parity/flow control,
/// one stop bit, and no terminal echo. Requires the OS `stty` utility.
/// Reads/writes return byte counts or IO errors; an idle read can return
/// `WouldBlock`, and a write can be partial. The caller handles retries/deadlines.
/// `close()` consumes the port and reports settings-restoration errors. Dropping
/// also restores settings, but cannot report errors or survive process termination.
/// The lock is advisory: only other clients using flock are kept out.
pub struct SerialPort {
    file: File,
    saved: SavedSettings,
    restore_on_drop: bool,
}

impl SerialPort {
    /// Preserve the current baud rate, including on USB modems that ignore it.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(OPEN_FLAGS)
            .open(path)?;
        file.try_lock()?;
        let saved = SavedSettings::capture(&file)?;
        let port = Self {
            file,
            saved,
            restore_on_drop: true,
        };
        TerminalOptions::new()
            .raw_mode()
            .set_data_bits(DataBits::Eight)
            .set_parity(Parity::None)
            .set_stop_bits(StopBits::One)
            .set_hardware_flow_control(false)
            .set_output_software_flow_control(false)
            .set_input_software_flow_control(false)
            .set_ignore_carrier(true)
            .set_receiver_enabled(true)
            .set_echo(false)
            .apply(&port.file)?;
        Ok(port)
    }

    pub fn close(mut self) -> io::Result<()> {
        self.saved.restore(&self.file)?;
        self.restore_on_drop = false;
        Ok(())
    }
}

impl Drop for SerialPort {
    fn drop(&mut self) {
        if self.restore_on_drop {
            let _ = self.saved.restore(&self.file);
        }
        // Closing the last descriptor releases its advisory lock.
    }
}

impl Read for SerialPort {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}
impl Write for SerialPort {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
