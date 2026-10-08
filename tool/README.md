# SIM probe

Requires Python 3.10+ on Linux/Pi; no third-party runtime packages. From the repository root:

```sh
python3 -m tool.probe --help
python3 -m tool.probe
python3 -m tool.probe --port /dev/ttyUSB2 --output /tmp/new-probe.json
```

Default discovery uses the stable SimTech interface-02 path. Verify the interface mapping before overriding it. Your account must have serial-port permission (normally `dialout`). Stop other modem clients first; they may not honor the probe's advisory lock.

The tool prints sanitized JSON: SIM status, signal, registration, SMS format and USB audio query support. It reads no SMS bodies and never sends, dials, deletes messages or changes persistent modem configuration. Serial settings are restored on exit. A failed query is inconclusive; command support does not prove working audio. An output file is created with mode 0600 and is never overwritten. Exit 0 means a report was obtained; inspect `query_errors` for partial results. Exit 1 means transport or output failure.

`device/interface.py` defines `SimDeviceInterface.get_status()`. `device/sim7600.py` owns SIM7600 commands/parsing; `device/at_transport.py` is its internal serial transport. The probe accepts any implementation of the interface. For a new modem/USB/network protocol, implement that contract and wire its selection into the tool; the report consumer does not need raw AT access. SMS/call methods will be added with their implementations rather than exposed as nonworking placeholders.

Tests (development machine):

```sh
python -m pip install pytest
python -m pytest -q tests/test_probe.py
```
