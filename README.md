# Pi SMS and voice gateway

First implementation slice: read-only SIM7600 diagnostics and an authenticated cloud diagnostic endpoint. SMS forwarding, call control/audio, and iOS are not implemented yet.

See [milestone-one steps and current blockers](docs/MILESTONE_ONE.md) and [cloud deployment](docs/CLOUD_SETUP.md).

## Device probe (on the Pi, Python 3, no extra packages)

Ensure no gateway daemon, minicom, or other modem process is using the serial port. This standalone probe takes an advisory exclusive lock; other tools may not honor it. It temporarily sets serial mode and restores it on exit. It does not read SMS bodies, dial, send, delete SMS, or change persistent modem settings.

```sh
python3 device/probe.py
```

Uses a stable SimTech interface-02 path; use `--port /dev/ttyUSB2` only after verifying interface mapping. Reports contain normalized readiness/signal values, no raw unsolicited notifications or subscriber identifiers. A rejected capability query is inconclusive, not proof the hardware lacks that capability.

## Proxy development

```sh
python3 -m venv .venv
. .venv/bin/activate
pip install -r requirements-dev.txt
export PROXY_DEVICE_TOKEN=$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')
export PROXY_ADMIN_TOKEN=$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')
uvicorn cloud.app:create_app --factory --host 127.0.0.1 --port 8000 --no-access-log
```

Keep tokens in a private secret manager or shell session; do not paste them into chat or commit them. Device and admin credentials are deliberately separate. `GET /healthz` is public; report submission requires the device token; reading the latest report requires the admin token. Only configured gateway `pi5` is accepted.

```sh
python -m pytest -q
```

This foundation uses SQLite for one gateway's latest sanitized report only. It is not the production PostgreSQL history database, an enrollment service, or a remote modem-command API. Reports older than 24 hours, future timestamps over 5 minutes, unknown fields, and bodies over 8 KiB are rejected. Newer reports replace older ones; identical retries are idempotent. Store full milestone test evidence locally under ignored `reports/` if needed.
