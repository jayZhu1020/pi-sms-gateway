# Cloud diagnostics proxy setup

This is a deployable foundation for one Pi, not the complete SMS/voice proxy. It has no call/SMS command endpoints, push provider, TURN media relay, or iOS client yet.

## Provider decision

A small Linux VM is the proposed starting point because the future voice relay needs networking beyond HTTP. DigitalOcean's published 1 GiB basic Droplet is $6/month as checked 2026-10-05; this is an initial diagnostics proposal, not capacity sizing for production voice. Confirm provider/account, region, monthly budget, and backup costs before provisioning. AWS or another existing Linux VM is also usable with the same container. Do not select the eventual China deployment region without route measurements.

[Provider pricing](https://www.digitalocean.com/products/droplets)

## Deployment steps after provider approval

1. Create a supported Linux VM, install Docker with Compose using the provider/OS instructions, and enroll it in your Tailscale network. Keep SSH private or limited to your administrator address. No public proxy port is required for this development stage.
2. Clone the reviewed/merged repository revision. Generate distinct random device/admin tokens using the README commands. Store them outside the repository, with access restricted to the deployment user.
3. Start from the repository root with those environment variables set:

   ```sh
   docker compose -f deploy/compose.yaml up -d --build
   curl --fail http://127.0.0.1:8000/healthz
   ```

4. The service is bound to host loopback and persists its latest report in a Docker volume. Access it through an SSH tunnel over Tailscale. On the Pi, replacing `CLOUD_TAILSCALE_IP` and `clouduser` with actual values:

   ```sh
   ssh -N -L 18080:127.0.0.1:8000 clouduser@CLOUD_TAILSCALE_IP
   ```

5. In another Pi terminal, create a fresh report and upload. The token file contains only the device token, permission mode 0600, provisioned through a private channel:

   ```sh
   mkdir -p reports
   python3 device/probe.py --output reports/probe.json
   python3 device/publish_report.py --url http://127.0.0.1:18080 --gateway pi5 --token-file /path/to/device-token --report reports/probe.json
   ```

   Use a new report filename per run; the probe refuses to overwrite one. A successful upload returns `accepted: true`. Inspect via `GET /v1/gateways/pi5/diagnostics` with the separate admin bearer token through your own tunnel. Check `age_seconds`; an old successful report is not a heartbeat.

6. Restart the proxy container and confirm the report persists. Check an invalid token is rejected. Test tunnel loss and retrying the same report. The uploader is one-shot and makes no claim of automatic reconnect or durable background delivery yet.

## Before public/mobile deployment

Add domain/TLS termination, rate limits/timeouts, revocable enrollment, token rotation, production PostgreSQL schemas and queues, backup/restore, and monitoring. An SSH tunnel is a development transport, not the production iOS API. Provision TURN and Apple push integration in their later reviewed milestones. The current SQLite volume is permission-restricted but not application-encrypted; use host storage encryption and store only diagnostic data.

Docker build/runtime must be verified on a Docker-capable host before declaring deployment complete. Run `docker compose -f deploy/compose.yaml down` to stop this stage while retaining the data volume; do not use `-v` unless intentionally deleting reports.
