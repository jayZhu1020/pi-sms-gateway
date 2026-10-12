//! Own a serial device and restore its terminal settings when finished.
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

// Open without waiting for carrier detection or taking control of this terminal.
#[cfg(target_os = "linux")]
const OPEN_FLAGS: i32 = 0x800 | 0x100; // O_NONBLOCK | O_NOCTTY
#[cfg(target_os = "macos")]
const OPEN_FLAGS: i32 = 0x4 | 0x20000;

fn settings(file: &File, args: &[&str]) -> io::Result<String> {
    // Pass the already-open device as stdin; no shell or second path lookup.
    let output = Command::new("/bin/stty")
        .args(args)
        .stdin(Stdio::from(file.try_clone()?))
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            "stty could not access/configure the terminal",
        ));
    }
    String::from_utf8(output.stdout).map_err(io::Error::other)
}

/// Owns one serial port for raw byte reads and writes.
///
/// `open(path)` takes a device path and returns a locked, nonblocking port or
/// an IO error. It configures 115200 baud, 8 data bits, no parity/flow control,
/// one stop bit, and no terminal echo. Requires the OS `stty` utility.
/// Reads/writes return byte counts or IO errors; an idle read can return
/// `WouldBlock`, and a write can be partial. The caller handles retries/deadlines.
/// `close()` consumes the port and reports settings-restoration errors. Dropping
/// also restores settings, but cannot report errors or survive process termination.
/// The lock is advisory: only other clients using flock are kept out.
pub struct SerialPort {
    file: File,
    saved: String,
    restore_on_drop: bool,
}

impl SerialPort {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(OPEN_FLAGS)
            .open(path)?;
        file.try_lock()?;
        let saved = settings(&file, &["-g"])?;
        let port = Self {
            file,
            saved,
            restore_on_drop: true,
        };
        settings(
            &port.file,
            &[
                "raw", "115200", "cs8", "-parenb", "-cstopb", "-crtscts", "-ixon", "-ixoff",
                "clocal", "cread", "-echo",
            ],
        )?;
        Ok(port)
    }

    pub fn close(mut self) -> io::Result<()> {
        settings(&self.file, &[self.saved.trim()])?;
        self.restore_on_drop = false;
        Ok(())
    }
}

impl Drop for SerialPort {
    fn drop(&mut self) {
        if self.restore_on_drop {
            let _ = settings(&self.file, &[self.saved.trim()]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, c_char, c_void};
    use std::os::fd::{AsRawFd, FromRawFd};
    #[link(name = "util")]
    unsafe extern "C" {
        fn openpty(
            master: *mut i32,
            slave: *mut i32,
            name: *mut u8,
            term: *const c_void,
            size: *const c_void,
        ) -> i32;
        fn ttyname(fd: i32) -> *mut c_char;
    }

    fn snapshot(file: &File) -> String {
        let saved = settings(file, &["-g"]).unwrap();
        #[cfg(target_os = "macos")]
        {
            // macOS toggles PENDIN (pending input), a kernel bookkeeping flag.
            let field = saved.split(':').find(|f| f.starts_with("lflag=")).unwrap();
            let flags = u32::from_str_radix(&field[6..], 16).unwrap() & !0x20000000;
            saved.replace(field, &format!("lflag={flags:x}"))
        }
        #[cfg(not(target_os = "macos"))]
        saved
    }

    #[test]
    fn exclusive_raw_io_and_settings_restore() {
        let (mut master_fd, mut slave_fd) = (-1, -1);
        // SAFETY: outputs are valid pointers; null optional inputs request defaults.
        assert_eq!(
            unsafe {
                openpty(
                    &mut master_fd,
                    &mut slave_fd,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            0
        );
        // SAFETY: openpty returned two new descriptors, each owned exactly once.
        let (mut master, slave) =
            unsafe { (File::from_raw_fd(master_fd), File::from_raw_fd(slave_fd)) };
        // SAFETY: live slave is a terminal; copy ttyname's C string before other calls.
        let name = unsafe { ttyname(slave.as_raw_fd()) };
        assert!(!name.is_null());
        let path = unsafe { CStr::from_ptr(name) }.to_str().unwrap().to_owned();
        let before = snapshot(&slave);
        let mut port = SerialPort::open(Path::new(&path)).unwrap();
        assert!(SerialPort::open(Path::new(&path)).is_err());
        assert_ne!(snapshot(&slave), before);
        assert_eq!(
            port.read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        master.write_all(b"OK\r\n").unwrap();
        let mut received = Vec::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while received.len() < 4 && std::time::Instant::now() < deadline {
            let mut bytes = [0; 4];
            match port.read(&mut bytes) {
                Ok(count) => received.extend_from_slice(&bytes[..count]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::yield_now(),
                Err(e) => panic!("{e}"),
            }
        }
        assert_eq!(received, b"OK\r\n");
        port.write_all(b"AT\r").unwrap();
        let mut sent = [0; 3];
        master.read_exact(&mut sent).unwrap();
        assert_eq!(&sent, b"AT\r");
        port.close().unwrap();
        assert_eq!(snapshot(&slave), before);
        drop(SerialPort::open(Path::new(&path)).unwrap());
        assert_eq!(snapshot(&slave), before);
    }

    #[test]
    fn rejects_missing_paths_and_non_terminal_devices() {
        assert!(SerialPort::open(Path::new("/nonexistent/serial-port")).is_err());
        assert!(SerialPort::open(Path::new("/dev/null")).is_err());
    }
}
