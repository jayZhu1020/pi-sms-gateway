import os
import pty
import threading
import time
import pytest
from device.probe import ATSession, ATError, parse_value


def test_fragmented_response_and_unsolicited_privacy():
    master, slave = pty.openpty()
    def modem():
        os.read(master, 1024)
        for part in [b'AT+CPIN?\r\n+CLIP: "private-number"\r\n+CP', b'IN: READY\r\n', b'OK\r\n']:
            os.write(master, part)
            time.sleep(.01)
    thread = threading.Thread(target=modem)
    try:
        with ATSession(os.ttyname(slave), timeout=1) as session:
            thread.start()
            lines = session.query('AT+CPIN?')
            assert parse_value('sim', lines) == 'READY'
            with pytest.raises(ValueError):
                session.query('ATD123;')
        thread.join(timeout=2)
    finally:
        os.close(master)
        os.close(slave)


def test_timeout_restores_port():
    import termios
    master, slave = pty.openpty()
    before = termios.tcgetattr(slave)
    try:
        with pytest.raises(TimeoutError):
            with ATSession(os.ttyname(slave), timeout=.1) as session:
                session.query('AT')
        after = termios.tcgetattr(slave)
        assert after[:3] == before[:3]
        assert after[4:] == before[4:]
        assert (after[3] & (termios.ICANON | termios.ECHO | termios.ISIG)) == (before[3] & (termios.ICANON | termios.ECHO | termios.ISIG))
    finally:
        os.close(master)
        os.close(slave)
