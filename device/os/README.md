# Device OS utilities

## Linux terminal settings

`terminal_options.rs` is an independent library wrapping `/bin/stty`.
`TerminalOptions` builds settings using named methods; only `apply` changes the
open terminal. It passes the existing file descriptor to stty without a shell.
No caller needs to supply command-line flags.

```rust
use terminal_options::{DataBits, Parity, SavedSettings, StopBits, TerminalOptions};

let saved = SavedSettings::capture(&file)?;
TerminalOptions::new()
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
    .set_echo(false)
    .apply(&file)?;
// Use the terminal; restore on failure as well as success in the owning layer.
saved.restore(&file)?;
```

Here `file` is an already-open terminal. Settings apply in call order; put raw
mode before overrides. Unsupported rates/configurations return IO errors; zero
baud is rejected because it means hangup. Failed application may partially change
settings. Saving/restoring is explicit; this library does not own the port or lock
it and does not automatically restore it. A future serial owner supplies cleanup.
