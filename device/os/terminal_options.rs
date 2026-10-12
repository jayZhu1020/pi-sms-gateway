//! Configure an open Linux terminal without exposing stty command-line flags.
use std::fs::File;
use std::io;
use std::process::{Command, Stdio};

fn run(file: &File, args: &[String]) -> io::Result<String> {
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

/// Saves a terminal's settings for explicit restoration later.
/// `capture(file)` reads settings; `restore(file)` reapplies them. Both return
/// IO errors on failure. Keep the original file open; no automatic restore occurs.
#[derive(PartialEq, Eq)]
pub struct SavedSettings(String);
impl SavedSettings {
    pub fn capture(file: &File) -> io::Result<Self> {
        run(file, &["-g".into()]).map(Self)
    }
    pub fn restore(&self, file: &File) -> io::Result<()> {
        run(file, &[self.0.trim().into()]).map(|_| ())
    }
}

pub enum DataBits {
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
}
pub enum Parity {
    None,
    Even,
    Odd,
}
pub enum StopBits {
    One,
    Two,
}

/// Builds terminal settings and changes the device only when `apply(file)` runs.
/// Setters accept typed values and return the builder for chaining. Settings are
/// applied in call order; later settings override earlier ones. Call `raw_mode`
/// first because it changes several settings. Empty options change nothing.
/// `apply` reports IO errors, including unsupported rates. Failure may partially
/// change settings; capture them first when rollback is needed. Linux only.
#[derive(Default)]
pub struct TerminalOptions {
    args: Vec<String>,
}
impl TerminalOptions {
    pub fn new() -> Self {
        Self::default()
    }
    fn flag(&mut self, name: &str, enabled: bool) -> &mut Self {
        self.args
            .push(format!("{}{name}", if enabled { "" } else { "-" }));
        self
    }
    /// Disable terminal text processing before applying individual overrides.
    pub fn raw_mode(&mut self) -> &mut Self {
        self.args.push("raw".into());
        self
    }
    /// Zero is rejected by apply because stty treats it as a hangup request.
    pub fn set_baud_rate(&mut self, baud: u32) -> &mut Self {
        self.args.push(baud.to_string());
        self
    }
    pub fn set_data_bits(&mut self, bits: DataBits) -> &mut Self {
        self.args.push(format!("cs{}", bits as u8));
        self
    }
    pub fn set_parity(&mut self, parity: Parity) -> &mut Self {
        self.flag("parenb", !matches!(parity, Parity::None));
        self.flag("parodd", matches!(parity, Parity::Odd))
    }
    pub fn set_stop_bits(&mut self, bits: StopBits) -> &mut Self {
        self.flag("cstopb", matches!(bits, StopBits::Two))
    }
    pub fn set_hardware_flow_control(&mut self, enabled: bool) -> &mut Self {
        self.flag("crtscts", enabled)
    }
    /// Honor received XON/XOFF characters to pause/resume our output.
    pub fn set_output_software_flow_control(&mut self, enabled: bool) -> &mut Self {
        self.flag("ixon", enabled)
    }
    /// Send XON/XOFF characters to regulate incoming data from the peer.
    pub fn set_input_software_flow_control(&mut self, enabled: bool) -> &mut Self {
        self.flag("ixoff", enabled)
    }
    pub fn set_ignore_carrier(&mut self, enabled: bool) -> &mut Self {
        self.flag("clocal", enabled)
    }
    pub fn set_receiver_enabled(&mut self, enabled: bool) -> &mut Self {
        self.flag("cread", enabled)
    }
    pub fn set_echo(&mut self, enabled: bool) -> &mut Self {
        self.flag("echo", enabled)
    }
    pub fn apply(&self, file: &File) -> io::Result<()> {
        if self.args.iter().any(|arg| arg == "0") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "baud must be nonzero",
            ));
        }
        if self.args.is_empty() {
            return Ok(());
        }
        run(file, &self.args).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translates_options_in_order() {
        let mut options = TerminalOptions::new();
        options
            .raw_mode()
            .set_baud_rate(115200)
            .set_data_bits(DataBits::Eight)
            .set_parity(Parity::None)
            .set_stop_bits(StopBits::One)
            .set_hardware_flow_control(false)
            .set_output_software_flow_control(false)
            .set_input_software_flow_control(false)
            .set_ignore_carrier(true)
            .set_receiver_enabled(true)
            .set_echo(false);
        assert_eq!(
            options.args.join(" "),
            "raw 115200 cs8 -parenb -parodd -cstopb -crtscts -ixon -ixoff clocal cread -echo"
        );
        options.args.clear();
        options
            .set_parity(Parity::Odd)
            .set_parity(Parity::Even)
            .set_stop_bits(StopBits::Two)
            .set_echo(true);
        assert_eq!(
            options.args.join(" "),
            "parenb parodd parenb -parodd cstopb echo"
        );
    }
    #[test]
    fn rejects_zero_rate_and_nonterminal() {
        let file = File::open("/dev/null").unwrap();
        assert!(TerminalOptions::new().apply(&file).is_ok());
        assert_eq!(
            TerminalOptions::new()
                .set_baud_rate(0)
                .apply(&file)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(SavedSettings::capture(&file).is_err());
        assert!(
            TerminalOptions::new()
                .set_baud_rate(115200)
                .apply(&file)
                .is_err()
        );
        assert!(SavedSettings("invalid".into()).restore(&file).is_err());
    }
}
