# pi_updater usage

Run on the development machine with Python 3.10+, SSH keys, a verified Pi host key, and `python -m pip install build`. The package sources/pyproject.toml come from PR #4; merge that before using `build`. No package builds or dependency downloads occur on the Pi. Pi prerequisites: Python 3.10+, venv/ensurepip, and serial access for your SSH user.

```sh
python -m tool.pi_updater build --source . --out /tmp/pi-sim-artifacts
python -m tool.pi_updater install --host jzhu@100.77.157.21 --host-key-alias pi5.local --wheel /tmp/pi-sim-artifacts/pi_sim_device-0.1.0-py3-none-any.whl
python -m tool.pi_updater update --host jzhu@100.77.157.21 --host-key-alias pi5.local --wheel /tmp/pi-sim-artifacts/pi_sim_device-0.1.0-py3-none-any.whl
python -m tool.pi_updater status --host jzhu@100.77.157.21 --host-key-alias pi5.local
python -m tool.pi_updater test --host jzhu@100.77.157.21 --host-key-alias pi5.local
python -m tool.pi_updater rollback --host jzhu@100.77.157.21 --host-key-alias pi5.local --release SHA256_FROM_STATUS
python -m tool.pi_updater uninstall --host jzhu@100.77.157.21 --host-key-alias pi5.local
```

`--host-key-alias` is optional; use it only when the alias identifies the same Pi. Default installation is `~/.local/share/pi-sim-device`, isolated from OS Python. For a temporary test, append `--root /tmp/pi-sim-device-test-UNIQUE` to every remote command (use lowercase letters/digits for UNIQUE). Only managed roots named pi-sim-device or pi-sim-device-test-* are accepted. An existing unmarked directory is refused.

Each wheel gets a SHA-256 release directory. Installation verifies transferred bytes, creates a venv, installs offline, then runs `sim-probe --help`. Only after success does the tool atomically switch `current`. Old releases remain available for rollback. `status` lists digest IDs; rerunning installation is idempotent. `test` performs read-only modem diagnostics, writes the report to stderr, and prints its completion on stdout; inspect `query_errors` for partial failures. Stop competing modem clients first.

Uninstall removes all managed application releases under that root. A parent lock file remains for safe serialization. No system packages, modem settings, or external history files are removed. There is no daemon/service in this first package. Commands exit nonzero on failure; a timeout during a remote operation requires checking `status` before retrying. Build artifacts are trusted software: hash verification provides transfer integrity, not publisher signatures. Native packages and dependency wheelhouses are not supported by this first installer.

Tests: `python -m pip install pytest`; `python -m pytest -q tests/test_updater.py`.
