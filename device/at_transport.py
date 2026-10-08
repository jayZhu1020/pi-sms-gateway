"""Internal bounded serial transport; not the device interface."""
import fcntl
import os
import select
import termios
import time
import tty

class ATError(RuntimeError):
    pass


class ATSession:
    def __init__(self, port, allowed, timeout=4):
        self.port, self.timeout, self.allowed = port, timeout, allowed
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
        if command not in self.allowed:
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
