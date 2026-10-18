# SIM7600 read-only probe

`sim_probe` queries SIM PIN readiness once. It sends only `AT+CPIN?`; it never
supplies a PIN/PUK, sends/deletes SMS, or places calls. Modem commands and response
validation live in `device/sim7600_probe.rs`, not in the CLI. This is a narrow
adapter operation, not yet a complete implementation of `SimDeviceInterface`.

## Build on a development machine or CI

On a Linux development machine with Bazelisk and a native toolchain:

```sh
bazel build //tool:sim_probe
bazel-bin/tool/sim_probe --help
```

The executable is Linux-only. A default
x86-64 Linux build cannot run on the Pi's ARM64 CPU. Build off-device with the Pi
configuration, then copy the executable to the Pi:

```sh
bazel build --config=pi //tool:sim_probe
install -m 755 bazel-bin/tool/sim_probe /tmp/sim_probe
scp /tmp/sim_probe USER@PI_HOST:/tmp/sim_probe
ssh USER@PI_HOST '/tmp/sim_probe --help'
```

The Pi requires `/bin/stty` and serial-device permissions. The updater remains
future work. Remove the temporary executable when finished.

## Run an ARM64 executable after installation

```sh
./sim_probe --help
./sim_probe /dev/ttyUSB2
```

`/dev/ttyUSB2` is an example: use the verified AT port or its stable by-id path,
not a guessed GPS/diagnostic port. The user needs read/write device access.
The runtime needs `/bin/stty`. SerialPort configures raw 8N1, takes an
advisory lock and restores terminal settings on close (best effort on errors).

Start with a synchronized modem connection and no other AT client. This initial
probe does not discover ports, drain old traffic, reset the modem or recover an
interrupted prior command. Restart/reinitialize the modem through your known
hardware procedure if the connection state is uncertain; reopening the port alone
is not synchronization. Do not automatically retry failed invocations.

Example output:

```text
SIM status: READY
Notifications observed: 0
```

Supported status lines are READY, SIM PIN and SIM PUK. Exit 0 means the query and
terminal restoration succeeded, including a locked SIM; read the status value.
Exit 1 means argument/open/exchange/validation/restoration failure. An OK without
status, unknown status or duplicate status is an error. A missing SIM may produce
a modem error. READY does not establish network registration or SMS reception.

The exchange has a five-second deadline and polls at millisecond intervals.
Opening/restoring the terminal and stty execution are outside that deadline.
Notifications seen before OK are counted but their contents are not printed or
stored. This is a diagnostic tool, not a persistent notification consumer.

Protocol reference: [SIMCom AT manual, CPIN section](https://files.waveshare.com/wiki/SIM7600G-H/SIM7500_SIM7600_Series_AT_Command_Manual_V3.00.pdf).
