//! Exercise SerialPort against a simulated Linux terminal, without modem hardware.
use serial_port::SerialPort;
use std::ffi::{CStr, c_char, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use terminal_options::{SavedSettings, TerminalOptions};

#[cfg_attr(not(target_env = "musl"), link(name = "util"))]
unsafe extern "C" {
    fn openpty(
        master: *mut i32,
        slave: *mut i32,
        name: *mut u8,
        term: *const c_void,
        size: *const c_void,
    ) -> i32;
    fn ttyname_r(fd: i32, name: *mut c_char, len: usize) -> i32;
    fn setsid() -> i32;
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
        let mut name = [0 as c_char; 128];
        // SAFETY: live terminal and writable buffer; ttyname_r allows parallel tests.
        assert_eq!(
            unsafe { ttyname_r(device.as_raw_fd(), name.as_mut_ptr(), name.len()) },
            0
        );
        // SAFETY: successful ttyname_r wrote a NUL-terminated name in this buffer.
        let device_path = PathBuf::from(unsafe { CStr::from_ptr(name.as_ptr()) }.to_str().unwrap());
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

#[test]
fn preserves_baud_unless_explicitly_requested() {
    let terminal = SimulatedTerminal::new();
    TerminalOptions::new()
        .set_baud_rate(38400)
        .apply(&terminal.device)
        .unwrap();
    let before = terminal.settings();
    let speed = || {
        let output = Command::new("/bin/stty")
            .arg("speed")
            .stdin(Stdio::from(terminal.device.try_clone().unwrap()))
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    let port = SerialPort::open(&terminal.device_path).unwrap();
    assert_eq!(speed(), "38400");
    port.close().unwrap();
    assert!(terminal.settings() == before);
    let port = SerialPort::open_with_baud_rate(&terminal.device_path, 115200).unwrap();
    assert_eq!(speed(), "115200");
    drop(port);
    assert!(terminal.settings() == before);
    assert!(SerialPort::open_with_baud_rate(&terminal.device_path, 0).is_err());
    assert!(terminal.settings() == before);
}

#[test]
fn does_not_acquire_controlling_terminal() {
    const CHILD_PORT: &str = "SERIAL_TEST_SESSION_PORT";
    if let Some(path) = std::env::var_os(CHILD_PORT) {
        // SAFETY: isolate this child process's session before opening the terminal.
        assert!(unsafe { setsid() } >= 0);
        assert!(File::open("/dev/tty").is_err());
        let port = SerialPort::open(Path::new(&path)).unwrap();
        assert!(File::open("/dev/tty").is_err());
        port.close().unwrap();
        return;
    }
    let terminal = SimulatedTerminal::new();
    let before = terminal.settings();
    let result = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "does_not_acquire_controlling_terminal",
            "--nocapture",
        ])
        .env(CHILD_PORT, &terminal.device_path)
        .status()
        .unwrap();
    assert!(result.success());
    assert!(terminal.settings() == before);
}
