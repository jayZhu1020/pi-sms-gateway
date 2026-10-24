"""Version-one incoming SMS contract; no transport, authentication or persistence."""

from dataclasses import asdict, dataclass
from datetime import datetime
import json
import re
from uuid import UUID

MAX_REQUEST_BYTES = 64 * 1024
MAX_BODY_BYTES = 16 * 1024
MAX_SEGMENTS = 32
# Revisions remain exact in browser JSON numbers.
MAX_REVISION = 2**53 - 1
_TIMESTAMP = re.compile(
    r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]{1,6})?(?:Z|[+-](?:[01][0-9]|2[0-3]):[0-5][0-9])"
)


class InvalidEvent(ValueError):
    """Only fixed, content-free reasons may cross this validation boundary."""


@dataclass(frozen=True, repr=False)
class SmsPayload:
    message_id: str
    revision: int
    direction: str
    sender_raw: str
    body: str | None
    encoding: str
    assembly_status: str
    segments_received: int
    segments_expected: int
    received_at: str
    carrier_timestamp: str | None


@dataclass(frozen=True, repr=False)
class SmsEvent:
    schema_version: int
    event_id: str
    gateway_id: str
    kind: str
    occurred_at: str
    payload: SmsPayload

    def to_wire(self) -> dict:
        """Explicit access to private wire data; never use this in operational logs."""
        return asdict(self)


def _object(value, fields: set[str], reason: str) -> dict:
    if type(value) is not dict or value.keys() != fields:
        raise InvalidEvent(reason)
    return value


def _integer(value, minimum: int, maximum: int, reason: str) -> int:
    # bool is an int subclass; JSON true must not become a version or counter.
    if type(value) is not int or not minimum <= value <= maximum:
        raise InvalidEvent(reason)
    return value


def _text(value, maximum: int, reason: str, *, empty: bool = False) -> str:
    if type(value) is not str or (not empty and not value):
        raise InvalidEvent(reason)
    try:
        size = len(value.encode("utf-8"))
    except UnicodeError:
        raise InvalidEvent(reason) from None
    if size > maximum:
        raise InvalidEvent(reason)
    return value


def _uuid(value, reason: str) -> str:
    try:
        if type(value) is not str or str(UUID(value)) != value or UUID(value).int == 0:
            raise ValueError
    except (ValueError, AttributeError):
        raise InvalidEvent(reason) from None
    return value


def _timestamp(value, reason: str) -> str:
    if (
        type(value) is not str
        or _TIMESTAMP.fullmatch(value) is None
        or value.endswith("-00:00")
    ):
        raise InvalidEvent(reason)
    try:
        datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        raise InvalidEvent(reason) from None
    return value


def _choice(value, options: tuple[str, ...], reason: str) -> str:
    if type(value) is not str or value not in options:
        raise InvalidEvent(reason)
    return value


def _pairs(pairs) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise InvalidEvent("duplicate JSON field")
        result[key] = value
    return result


def _constant(_):
    raise InvalidEvent("non-finite JSON number")


def parse_event(raw: bytes, *, authenticated_gateway_id: str) -> SmsEvent:
    """Validate untrusted JSON against an identity supplied by a trusted caller.

    This function does not authenticate credentials. The eventual HTTP handler
    must authenticate first and enforce the request limit while reading the body.
    No duplicate suppression or durable acknowledgment occurs in this parser.
    """
    gateway = _uuid(authenticated_gateway_id, "invalid authenticated gateway identity")
    if type(raw) is not bytes or len(raw) > MAX_REQUEST_BYTES:
        raise InvalidEvent("invalid request size or type")
    try:
        data = json.loads(
            raw.decode("utf-8"), object_pairs_hook=_pairs, parse_constant=_constant
        )
    except (ValueError, UnicodeError, RecursionError):
        # JSON parser exceptions can retain the entire private document.
        raise InvalidEvent("invalid JSON document") from None
    event = _object(data, set(SmsEvent.__dataclass_fields__), "invalid event fields")
    _integer(event["schema_version"], 1, 1, "unsupported schema version")
    _uuid(event["event_id"], "invalid event identity")
    if _uuid(event["gateway_id"], "invalid gateway identity") != gateway:
        raise InvalidEvent("gateway identity mismatch")
    kind = _choice(
        event["kind"], ("sms.received", "sms.updated"), "unsupported event kind"
    )
    _timestamp(event["occurred_at"], "invalid observation timestamp")
    payload = _object(
        event["payload"], set(SmsPayload.__dataclass_fields__), "invalid payload fields"
    )
    _uuid(payload["message_id"], "invalid message identity")
    revision = _integer(payload["revision"], 1, MAX_REVISION, "invalid revision")
    if (kind == "sms.received") != (revision == 1):
        raise InvalidEvent("event kind and revision disagree")
    _choice(payload["direction"], ("inbound",), "unsupported message direction")
    _text(payload["sender_raw"], 128, "invalid sender")
    encoding = _choice(
        payload["encoding"], ("gsm7", "ucs2", "unsupported"), "unsupported encoding"
    )
    status = _choice(
        payload["assembly_status"],
        ("complete", "partial", "unsupported"),
        "invalid assembly status",
    )
    received = _integer(
        payload["segments_received"], 1, MAX_SEGMENTS, "invalid received segment count"
    )
    expected = _integer(
        payload["segments_expected"], 1, MAX_SEGMENTS, "invalid expected segment count"
    )
    if received > expected:
        raise InvalidEvent("inconsistent segment counts")
    if status == "unsupported":
        if payload["body"] is not None:
            raise InvalidEvent("unsupported message must omit decoded body")
    else:
        _text(payload["body"], MAX_BODY_BYTES, "invalid message body", empty=True)
        if encoding == "unsupported" or (status == "complete") != (
            received == expected
        ):
            raise InvalidEvent("inconsistent assembly metadata")
    _timestamp(payload["received_at"], "invalid reception timestamp")
    if payload["carrier_timestamp"] is not None:
        _timestamp(payload["carrier_timestamp"], "invalid carrier timestamp")
    return SmsEvent(**{**event, "payload": SmsPayload(**payload)})
