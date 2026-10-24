"""Synthetic wire contracts for device uploads and the future browser API."""

from contextlib import redirect_stderr, redirect_stdout
from dataclasses import FrozenInstanceError
from io import StringIO
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from cloud.events import InvalidEvent, MAX_BODY_BYTES, MAX_REQUEST_BYTES, parse_event
from cloud.validate_event import main

FIXTURES = Path(__file__).with_name("fixtures")
GATEWAY = "00000000-0000-4000-8000-000000000002"


def fixture(name="complete"):
    return json.loads((FIXTURES / f"{name}.json").read_bytes())


def parse(data):
    return parse_event(json.dumps(data).encode(), authenticated_gateway_id=GATEWAY)


class EventTest(unittest.TestCase):
    def rejects(self, data):
        with self.assertRaises(InvalidEvent) as caught:
            parse(data)
        self.assertNotIn("PRIVATE", str(caught.exception))
        return str(caught.exception)

    def test_complete_partial_and_unsupported_fixtures_round_trip(self):
        for name in ("complete", "partial", "unsupported"):
            with self.subTest(name=name):
                data = fixture(name)
                self.assertEqual(parse(data).to_wire(), data)

    def test_unicode_sender_body_and_carrier_offset_preserved(self):
        data = fixture()
        data["payload"].update(
            sender_raw="Example 中文",
            body="你好\n<example>&\u0000",
            encoding="ucs2",
            carrier_timestamp="2026-10-24T08:00:00.123456+08:00",
        )
        self.assertEqual(parse(data).to_wire(), data)

    def test_updates_keep_message_identity_but_have_new_event_and_revision(self):
        first = parse(fixture("partial"))
        data = fixture()
        data.update(kind="sms.updated", event_id="00000000-0000-4000-8000-000000000004")
        data["payload"].update(revision=2, segments_received=2, segments_expected=2)
        updated = parse(data)
        self.assertEqual(first.payload.message_id, updated.payload.message_id)
        self.assertNotEqual(first.event_id, updated.event_id)
        self.assertEqual(updated.payload.revision, 2)
        data["payload"]["revision"] = 1
        self.rejects(data)
        data.update(kind="sms.received")
        data["payload"]["revision"] = 2
        self.rejects(data)

    def test_payloads_are_immutable_and_repr_excludes_private_content(self):
        data = fixture()
        data["payload"].update(sender_raw="PRIVATE", body="PRIVATE")
        event = parse(data)
        for value in (event, event.payload):
            self.assertNotIn("PRIVATE", repr(value))
        with self.assertRaises(FrozenInstanceError):
            event.payload.body = "changed"
        copy = event.to_wire()
        copy["payload"]["body"] = "changed"
        self.assertEqual(event.payload.body, "PRIVATE")

    def test_exact_fields_and_object_shapes(self):
        for target in ("event", "payload"):
            for mutation in ("missing", "extra"):
                data = fixture()
                obj = data if target == "event" else data["payload"]
                if mutation == "missing":
                    del obj["schema_version" if target == "event" else "body"]
                else:
                    obj["PRIVATE"] = "PRIVATE"
                self.rejects(data)
        for obj in (None, [], "PRIVATE", 1):
            self.rejects(obj)
            data = fixture()
            data["payload"] = obj
            self.rejects(data)

    def test_version_kind_and_inbound_only(self):
        for key, value in (
            ("schema_version", True),
            ("schema_version", 1.0),
            ("schema_version", 2),
            ("kind", "sms.sent"),
        ):
            data = fixture()
            data[key] = value
            self.rejects(data)
        data = fixture()
        data["payload"]["direction"] = "outbound"
        self.rejects(data)

    def test_canonical_nonzero_ids_and_authenticated_gateway_binding(self):
        for target, key in (
            ("event", "event_id"),
            ("event", "gateway_id"),
            ("payload", "message_id"),
        ):
            for value in (
                None,
                7,
                "PRIVATE",
                "0" * 32,
                "00000000000040008000000000000001",
                "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            ):
                data = fixture()
                (data if target == "event" else data["payload"])[key] = value
                self.rejects(data)
        data = fixture()
        data["gateway_id"] = "00000000-0000-4000-8000-000000000005"
        self.assertEqual(self.rejects(data), "gateway identity mismatch")
        with self.assertRaises(InvalidEvent):
            parse_event(
                json.dumps(fixture()).encode(), authenticated_gateway_id="PRIVATE"
            )

    def test_sender_and_body_utf8_byte_limits(self):
        data = fixture()
        data["payload"]["sender_raw"] = "é" * 64
        data["payload"]["body"] = "你" * (MAX_BODY_BYTES // 3) + "x"
        parse(data)
        data["payload"]["body"] += "x"
        self.rejects(data)
        data["payload"]["body"] = ""
        parse(data)
        data["payload"]["sender_raw"] += "x"
        self.rejects(data)
        for value in ("", None, [], "\ud800"):
            data["payload"]["sender_raw"] = value
            self.rejects(data)
        data = fixture()
        for value in (None, 1, [], "\ud800"):
            data["payload"]["body"] = value
            self.rejects(data)

    def test_request_byte_limit_before_json_parsing(self):
        raw = json.dumps(fixture()).encode()
        padded = raw + b" " * (MAX_REQUEST_BYTES - len(raw))
        parse_event(padded, authenticated_gateway_id=GATEWAY)
        for value in (padded + b" ", b"x" * (MAX_REQUEST_BYTES + 1), raw.decode()):
            with self.assertRaises(InvalidEvent):
                parse_event(value, authenticated_gateway_id=GATEWAY)

    def test_json_rejects_duplicates_nonfinite_invalid_utf8_and_deep_nesting(self):
        raw = json.dumps(fixture()).encode()
        values = [
            b"{",
            b"\xff",
            b"{} {}",
            b"NaN",
            b"Infinity",
            b"-Infinity",
            b"[" * 2000 + b"]" * 2000,
            raw.replace(
                b'"schema_version": 1', b'"schema_version": 1, "schema_version": 2'
            ),
            raw.replace(b'"revision": 1', b'"revision": 1, "revision": 2'),
            raw.replace(b'"revision": 1', b'"revision": NaN'),
        ]
        for value in values:
            with self.assertRaises(InvalidEvent) as caught:
                parse_event(value, authenticated_gateway_id=GATEWAY)
            self.assertIn(
                str(caught.exception), ("invalid JSON document", "invalid event fields")
            )

    def test_timestamps_require_real_dates_and_explicit_valid_offsets(self):
        invalid = [
            "PRIVATE",
            None,
            1,
            "2026-02-30T00:00:00Z",
            "2026-10-24",
            "2026-10-24T00:00:00",
            "2026-10-24 00:00:00Z",
            "2026-10-24T00:00:60Z",
            "2026-10-24T00:00:00+24:00",
            "2026-10-24T00:00:00+01:99",
            "2026-10-24T00:00:00.1234567Z",
            "2026-10-24T00:00:00-00:00",
        ]
        for location, key in (
            ("event", "occurred_at"),
            ("payload", "received_at"),
            ("payload", "carrier_timestamp"),
        ):
            for value in invalid:
                if key == "carrier_timestamp" and value is None:
                    continue
                data = fixture()
                (data if location == "event" else data["payload"])[key] = value
                self.rejects(data)

    def test_counters_reject_bools_floats_and_bounds(self):
        for key in ("revision", "segments_received", "segments_expected"):
            maximum = 2**53 - 1 if key == "revision" else 32
            for value in (True, False, 1.0, "1", 0, -1, maximum + 1):
                data = fixture()
                data["payload"][key] = value
                self.rejects(data)
        data = fixture()
        data["kind"] = "sms.updated"
        data["payload"]["revision"] = 2**53 - 1
        self.assertEqual(parse(data).payload.revision, 2**53 - 1)
        data = fixture()
        data["payload"].update(segments_received=32, segments_expected=32)
        parse(data)

    def test_assembly_metadata_cannot_misrepresent_completeness(self):
        for updates in (
            {"segments_received": 2},
            {"segments_expected": 2},
            {"assembly_status": "partial"},
            {"assembly_status": "PRIVATE"},
            {"encoding": "PRIVATE"},
            {"encoding": "unsupported"},
            {"assembly_status": "unsupported"},
        ):
            data = fixture()
            data["payload"].update(updates)
            self.rejects(data)
        data = fixture("unsupported")
        data["payload"]["encoding"] = "gsm7"  # Known coding, undecodable content.
        parse(data)

    def test_cli_reports_status_without_private_payloads(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "event.json"
            data = fixture()
            data["payload"]["body"] = "PRIVATE"
            for raw, identity, expected in (
                (json.dumps(data).encode(), GATEWAY, 0),
                (b'{"PRIVATE":', GATEWAY, 1),
                (b"x" * (MAX_REQUEST_BYTES + 1), GATEWAY, 1),
                (json.dumps(data).encode(), "PRIVATE", 1),
            ):
                path.write_bytes(raw)
                out, err = StringIO(), StringIO()
                with patch(
                    "sys.argv", ["validate_event", str(path), "--gateway-id", identity]
                ), redirect_stdout(out), redirect_stderr(err):
                    self.assertEqual(main(), expected)
                self.assertNotIn("PRIVATE", out.getvalue() + err.getvalue())
            path.unlink()
            with patch(
                "sys.argv", ["validate_event", str(path), "--gateway-id", GATEWAY]
            ), redirect_stderr(StringIO()):
                self.assertEqual(main(), 1)


if __name__ == "__main__":
    unittest.main()
