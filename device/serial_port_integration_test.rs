//! Exercise SerialPort against a simulated Linux terminal, without modem hardware.
use serial_port::SerialPort;
use std::ffi::{CStr, c_char, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use terminal_options::SavedSettings;

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

/// A connected pair: SerialPort opens device_path; the test acts as its peer.
/// Keeping both files alive preserves the terminal until the fixture is dropped.
struct SimulatedTerminal {
    peer: File,
    device: File,
    device_path: PathBuf,
}
impl SimulatedTerminal {
    fn new() -> Self {
        let (mut peer_fd, mut device_fd) = (-1, -1);
        // SAFETY: valid output pointers; null optional inputs select OS defaults.
        let result = unsafe {
            openpty(
                &mut peer_fd,
                &mut device_fd,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert_eq!(result, 0, "create simulated terminal");
        // SAFETY: successful openpty returned fresh descriptors, owned once here.
        let (peer, device) = unsafe { (File::from_raw_fd(peer_fd), File::from_raw_fd(device_fd)) };
        // SAFETY: device is a live terminal; copy its name before another OS call.
        let name = unsafe { ttyname(device.as_raw_fd()) };
        assert!(!name.is_null());
        let device_path = PathBuf::from(unsafe { CStr::from_ptr(name) }.to_str().unwrap());
        Self {
            peer,
            device,
            device_path,
        }
    }
    fn settings(&self) -> SavedSettings {
        SavedSettings::capture(&self.device).unwrap()
    }
    fn send_to_port(&mut self, bytes: &[u8]) {
        self.peer.write_all(bytes).unwrap();
    }
    fn expect_from_port(&mut self, expected: &[u8]) {
        let mut received = vec![0; expected.len()];
        self.peer.read_exact(&mut received).unwrap();
        assert_eq!(received, expected);
    }
}

#[test]
fn exclusive_raw_io_and_settings_restore() {
    let mut terminal = SimulatedTerminal::new();
    let before = terminal.settings();
    let mut port = SerialPort::open(&terminal.device_path).unwrap();
    assert!(SerialPort::open(&terminal.device_path).is_err());
    assert!(terminal.settings() != before);
    assert_eq!(
        port.read(&mut [0; 1]).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );

    terminal.send_to_port(b"OK\r\n");
    let mut received = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(1);
    while received.len() < 4 && Instant::now() < deadline {
        let mut bytes = [0; 4];
        match port.read(&mut bytes) {
            Ok(count) => received.extend_from_slice(&bytes[..count]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::yield_now(),
            Err(e) => panic!("{e}"),
        }
    }
    assert_eq!(received, b"OK\r\n");
    port.write_all(b"AT\r").unwrap();
    terminal.expect_from_port(b"AT\r");

    port.close().unwrap();
    assert!(terminal.settings() == before);
    drop(SerialPort::open(&terminal.device_path).unwrap());
    assert!(terminal.settings() == before);
}

#[test]
fn rejects_missing_paths_and_non_terminal_devices() {
    assert!(SerialPort::open(Path::new("/nonexistent/serial-port")).is_err());
    assert!(SerialPort::open(Path::new("/dev/null")).is_err());
}
