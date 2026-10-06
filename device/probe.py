"""Read-only SIM7600 diagnostics. No SMS contents, identifiers, or call commands."""
import argparse
import fcntl
import glob
import json
import os
from pathlib import Path
import re
import select
import termios
import time
import tty
from datetime import datetime, timezone
from uuid import uuid4

QUERIES = {
    "sim": "AT+CPIN?", "signal": "AT+CSQ", "registration": "AT+CEREG?",
    "sms_format": "AT+CMGF?", "usb_audio": "AT+CPCMREG=?",
}


class ATError(Exception):
    pass


class ATSession:
    def __init__(self, port, timeout=4):
        self.port, self.timeout = port, timeout
        self.fd = None
        self.old = None

    def __enter__(self):
        self.fd = os.open(self.port, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        try:
            fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.old = termios.tcgetattr(self.fd)
            tty.setraw(self.fd)
            settings = termios.tcgetattr(self.fd)
            settings[4] = settings[5] = termios.B115200
            settings[2] |= termios.CLOCAL | termios.CREAD
            settings[2] &= ~termios.CRTSCTS
            termios.tcsetattr(self.fd, termios.TCSANOW, settings)
        except BaseException:
            self.__exit__(None, None, None)
            raise
        return self

    def __exit__(self, *args):
        try:
            if self.old is not None:
                termios.tcsetattr(self.fd, termios.TCSANOW, self.old)
        finally:
            if self.fd is not None:
                os.close(self.fd)
                self.fd = None

    def query(self, command):
        if command not in {"AT", *QUERIES.values()}:
            raise ValueError("Only diagnostic queries are allowed")
        # Standalone diagnostics only: do not run alongside a gateway daemon.
        termios.tcflush(self.fd, termios.TCIFLUSH)
        pending = (command + "\r").encode()
        deadline = time.monotonic() + self.timeout
        buffer, lines = b"", []
        while time.monotonic() < deadline:
            readable, writable, _ = select.select(
                [self.fd], [self.fd] if pending else [], [], 0.1)
            if writable:
                try:
                    pending = pending[os.write(self.fd, pending):]
                except BlockingIOError:
                    pass
            if readable:
                try:
                    chunk = os.read(self.fd, 4096)
                except BlockingIOError:
                    continue
                if not chunk:
                    raise ATError("Modem disconnected")
                buffer += chunk
                if len(buffer) > 16384 or len(lines) > 128:
                    raise ATError("Modem response exceeded limit")
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    line = line.decode("ascii", errors="replace").strip()
                    if line == "OK":
                        return lines
                    if line == "ERROR" or line.startswith(("+CME ERROR", "+CMS ERROR")):
                        raise ATError("Modem rejected query")
                    if line and line != command:
                        lines.append(line)
        # Abort the session after a timeout; a late OK must not satisfy a new query.
        raise TimeoutError("Modem response timed out")


def discover_port():
    matches = glob.glob('/dev/serial/by-id/*SimTech*if02-port0')
    if len(matches) != 1:
        raise ATError("Expected one SimTech AT interface; pass --port explicitly")
    return matches[0]


def parse_value(name, lines):
    patterns = {
        "sim": r"\+CPIN: (READY|SIM PIN|SIM PUK)$",
        "signal": r"\+CSQ: (\d+),\d+$",
        "registration": r"\+CEREG: \d+,(\d+)(?:,.*)?$",
        "sms_format": r"\+CMGF: ([01])$",
        "usb_audio": r"\+CPCMREG: .*",
    }
    for line in lines:
        match = re.fullmatch(patterns[name], line)
        if match:
            if name == "usb_audio":
                return "supported"
            return match.group(1) if name == "sim" else int(match.group(1))
    return None


def probe(port=None):
    report = {"schema_version": 1, "report_id": str(uuid4()),
              "observed_at": datetime.now(timezone.utc).isoformat(),
              "modem_responding": False, "sim": None, "signal": None,
              "registration": None, "sms_format": None, "usb_audio": None,
              "query_errors": []}
    with ATSession(port or discover_port()) as modem:
        modem.query("AT")
        report["modem_responding"] = True
        for name, command in QUERIES.items():
            try:
                lines = modem.query(command)
                report[name] = parse_value(name, lines)
                if report[name] is None:
                    report["query_errors"].append(name)
            except ATError:
                report["query_errors"].append(name)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port")
    parser.add_argument("--output", help="Optional private JSON report path")
    args = parser.parse_args()
    try:
        report = probe(args.port)
    except (ATError, OSError, TimeoutError):
        parser.exit(1, "Probe failed: port unavailable, locked, disconnected, or timed out.\n")
    encoded = json.dumps(report, indent=2) + "\n"
    if args.output:
        # Never overwrite an existing report, follow a symlink, or create world-readable output.
        fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as stream:
            stream.write(encoded)
    print(encoded, end="")


if __name__ == "__main__":
    main()
