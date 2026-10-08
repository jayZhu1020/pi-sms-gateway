"""Read-only SIM diagnostics; run with python3 -m tool.probe from the repo root."""
import argparse
from dataclasses import asdict
from datetime import datetime, timezone
import json
import os
from uuid import uuid4
from device.interface import SimDeviceInterface
from device.sim7600 import Sim7600Device


def probe(device: SimDeviceInterface):
    return {"schema_version": 1, "report_id": str(uuid4()),
            "observed_at": datetime.now(timezone.utc).isoformat(),
            **asdict(device.get_status())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", help="AT port override; default: discover stable USB interface 02")
    parser.add_argument("--output", help="New JSON file (0600); existing files are never overwritten")
    args = parser.parse_args()
    try:
        encoded = json.dumps(probe(Sim7600Device(args.port)), indent=2) + "\n"
        if args.output:
            fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "w") as stream:
                stream.write(encoded)
    except (OSError, RuntimeError, TimeoutError):
        parser.exit(1, "Probe failed: check port ownership, connection, and output path.\n")
    print(encoded, end="")


if __name__ == "__main__":
    main()
