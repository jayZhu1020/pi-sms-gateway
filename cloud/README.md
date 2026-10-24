# Incoming SMS event contract

The cloud receives device events and exposes message history to an authenticated
website. Native mobile applications are deferred. Hosting and database selection
remain separate from this contract; this directory does not yet serve HTTP or
store messages.

`events.parse_event(raw, authenticated_gateway_id=...)` validates UTF-8 JSON bytes
and returns immutable `SmsEvent` / `SmsPayload` objects. The identity argument must
come from a trusted authentication layer. Parsing does not authenticate a token,
suppress duplicates, allocate identities or acknowledge durable storage.

The version-one envelope contains exactly `schema_version`, `event_id`,
`gateway_id`, `kind`, `occurred_at` and `payload`. IDs are nonzero UUIDs in lowercase
hyphenated form. The device generates stable IDs before uploading and reuses the
same event on a retry. Gateway ID must match the authenticated identity.

The payload contains exactly:

| Field | Meaning |
|---|---|
| `message_id`, `revision` | Stable message identity and positive revision (at most 2^53−1 for exact browser JSON numbers) |
| `direction` | `inbound` |
| `sender_raw` | Original sender, including alphanumeric senders/short codes; 1–128 UTF-8 bytes |
| `body` | Decoded text, including empty text, or null for unsupported content |
| `encoding` | `gsm7`, `ucs2` or `unsupported` |
| `assembly_status` | `complete`, `partial` or `unsupported` |
| `segments_received`, `segments_expected` | Counts from 1 to 32; received cannot exceed expected |
| `received_at` | Device reception timestamp |
| `carrier_timestamp` | Carrier timestamp when available, otherwise null |

`sms.received` has revision 1; `sms.updated` has revision 2 or greater. A partial
message has fewer received than expected segments; a complete message has equal
counts. Both require a decoded body and known encoding. Unsupported content has
a null body, with known or unsupported encoding. Raw bytes remain on the device;
this event does not carry a PDU. Distinct deliveries retain distinct message IDs,
even with identical text. Retry/revision/conflict handling belongs to ingestion.

Requests are limited to 64 KiB; decoded text bodies to 16 KiB of UTF-8. Integer
fields reject booleans and floating-point values. JSON rejects duplicate keys,
non-finite numbers, invalid Unicode and unexpected/missing fields. Timestamps
use `YYYY-MM-DDTHH:MM:SS[.ffffff]Z` or an explicit numeric offset. Fractions have
1–6 digits. Dates must be valid; leap seconds and unknown `-00:00` offsets are
not accepted. Carrier offsets and original text are preserved without normalization.

Validation errors use fixed reasons without copying input content. Event/payload
representations omit fields; `to_wire()` explicitly exposes private data and must
not be used in operational logs. The future HTTP layer must authenticate callers,
bound the body while reading, avoid private-response caching and render SMS text
as text rather than HTML.

## Inspect a private event file

```sh
bazel run //cloud:validate_event -- /absolute/path/event.json \
  --gateway-id YOUR_GATEWAY_UUID
```

The command prints only validation status and exits 0 for valid input or 1 for
validation/file errors. Its gateway argument is a local consistency check, not
credential authentication. Files in `fixtures/` contain synthetic wire examples
for complete, partial and unsupported messages.
