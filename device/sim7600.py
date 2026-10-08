"""SIM7600 adapter. All modem-specific commands and parsing stay here."""
import glob
import re
from device.interface import DeviceStatus, SimDeviceInterface
from device.at_transport import ATError, ATSession

QUERIES = {"sim": "AT+CPIN?", "signal": "AT+CSQ", "registration": "AT+CEREG?",
           "sms_format": "AT+CMGF?", "usb_audio": "AT+CPCMREG=?"}


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



class Sim7600Device(SimDeviceInterface):
    def __init__(self, port=None):
        self.port = port

    def get_status(self) -> DeviceStatus:
        matches = glob.glob('/dev/serial/by-id/*SimTech*if02-port0')
        port = self.port
        if not port:
            if len(matches) != 1:
                raise ATError("Expected one SimTech AT interface; use --port")
            port = matches[0]
        status = DeviceStatus()
        with ATSession(port, {"AT", *QUERIES.values()}) as modem:
            modem.query("AT")
            status.modem_responding = True
            for name, command in QUERIES.items():
                try:
                    value = parse_value(name, modem.query(command))
                    setattr(status, name, value)
                    if value is None:
                        status.query_errors.append(name)
                except ATError:
                    status.query_errors.append(name)
        return status
