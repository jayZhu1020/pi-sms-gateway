"""Hardware-independent diagnostic contract; no raw transport or AT commands."""
from abc import ABC, abstractmethod
from dataclasses import dataclass, field


@dataclass
class DeviceStatus:
    modem_responding: bool = False
    sim: str | None = None
    signal: int | None = None
    registration: int | None = None
    sms_format: int | None = None
    usb_audio: str | None = None
    query_errors: list[str] = field(default_factory=list)


class SimDeviceInterface(ABC):
    @abstractmethod
    def get_status(self) -> DeviceStatus:
        """Read current status without cellular side effects; raise on transport failure.

        Unknown/unsupported fields are None, with names in query_errors.
        Each adapter owns transport acquisition, timeout handling and cleanup.
        """
