# Remote SIM SMS and Voice Gateway — Design for Review

Status: proposed architecture, updated 2026-10-05. Implementation backlog: [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md).

## Required product outcomes

The physical China Mobile SIM stays in a Raspberry Pi 5 / Waveshare SIM7600G-H gateway. From a dedicated iOS app, the user must be able to:

1. Receive SMS sent to that SIM number.
2. Receive and answer calls to that number, with live two-way audio.
3. Send SMS through that SIM number.
4. Make calls through that SIM number, with live two-way audio.

The app stores and synchronizes SMS conversations and call history with a familiar Messages/Phone-style experience. All four operations are release requirements. SMS can ship as an intermediate milestone, but does not complete this project. This design supersedes the previous SMS-only plan and its Pi 4 purchasing recommendations; those remain available in Git history.

Initial scope: one owner, one Pi, one SIM, one active cellular call at a time, and one enrolled iPhone. No MMS, RCS, conference calls, voicemail service, or call recording in the first release. The app owns its history; importing or replacing the system Messages database is not a requirement. CallKit provides system call interaction, not the authoritative history store.

## Architecture: three components

```text
External phone <-- cellular SMS / voice --> China Mobile SIM + Waveshare
                                                         |
                                               USB control + audio*
                                                         |
                                             1. Pi device proxy
                                                         |
                              outbound authenticated TLS / WebSocket
                                                         |
                                             2. Cloud proxy
                                        API + history + signaling
                                        APNs provider + TURN relay
                                                         |
                                           HTTPS / WebSocket / push
                                                         |
                                             3. Native iOS app

Voice media: Pi <== encrypted WebRTC audio via cloud TURN ==> iPhone
* The exact modem-to-Pi audio path must pass the first hardware gate.
```

Home Ethernet/Wi-Fi carries the Pi's Internet traffic. The SIM supplies cellular SMS and voice; it is not used as the gateway's Internet uplink. Tailscale is the maintenance path for developers, not a dependency for ordinary app operation. The Pi opens its own cloud connections; no home-router port forwarding is required.

### Current evidence

| Item | Observed status |
|---|---|
| Pi 5, headless OS, SSH | Working |
| Tailscale SSH, service enabled at boot | Verified |
| Cooling fan | Detected; user confirmed rotation |
| SIM7600G-H USB and AT control | Working |
| SIM / network | Last probe: SIM READY, LTE roaming registration, carrier reported AT&T CMCC |
| Actual inbound / outbound SMS | Not yet verified |
| Actual inbound / outbound voice | Not yet verified |
| Modem-to-Pi audio capture and playback | Not yet verified |
| Cloud service / iOS app | Proposed, not implemented |

Registration does not prove SMS delivery or voice availability. US roaming tests do not establish behavior on the final China network. Validate both environments.

## 1. Device-side proxy server

### Responsibilities

- Own the modem serial interface in a single service. Serialize commands, parse asynchronous notifications, correlate responses, and handle timeouts without multiple processes competing for the same port.
- Discover the modem by stable USB identity, rather than assuming ttyUSB numbering never changes. Track SIM presence, registration, signal, modem firmware, and restart state.
- Receive SMS, decode Unicode and multipart PDU messages, persist before acknowledging cloud transfer or deleting from modem storage, and reconcile storage after reconnect/restart.
- Submit outgoing SMS, record per-segment submission results and delivery reports where supported, and prevent repeated API requests from creating new sends.
- Control incoming/outgoing calls: ring, caller ID when available, dial, answer, reject, hang up, DTMF, and disconnect cause. Serialize concurrent call actions and enforce one active call.
- Bridge modem audio into a WebRTC session, with capture/playback, resampling, Opus encoding, jitter handling, and audio-route health checks.
- Persist local SMS, call events, command receipts, and cloud-transfer outbox in SQLite. Run under systemd and reconnect with bounded backoff.

Proposed implementation: Python async control service, SQLite, and a separate audio worker using a maintained WebRTC/media stack. The initial audio experiment selects the concrete library and hardware path before committing to that implementation. IPC between control and audio workers remains local.

### Voice hardware gate

First demonstrate cellular incoming/outgoing calls on the actual SIM and modem firmware, then prove simultaneous capture and playback from Linux. Prefer a supported USB digital-audio path if this board/firmware exposes one. The USB AT ports alone do not demonstrate audio support.

If USB audio cannot be made reliable, evaluate documented PCM/I2S integration or a correctly engineered analog interface to the modem audio connector. Verify electrical levels, pinout, isolation, and echo behavior before purchasing or wiring alternatives. If the current modem cannot meet the requirement, propose a compatible replacement; do not remove voice from scope.

Waveshare documents call examples for this board family, but they are not evidence of working audio on our unit. [Board documentation](https://www.waveshare.com/wiki/SIM7600G-H_4G_HAT_%28B%29)

### Device API and operating boundaries

The production command channel is the authenticated outbound cloud connection. A localhost diagnostic API exposes health, queue state, and sanitized call state; remote diagnostics use an SSH tunnel. No public AT-command or arbitrary-shell endpoint.

Commands contain a schema version, command ID, device ID, operation, payload, and expiry. The Pi persists acceptance before executing. Call actions also include call ID and expected state/version; delayed commands for ended calls are rejected. A call command that expires while offline is never executed later.

## 2. Cloud proxy and history service

### Responsibilities

- Authenticate the user, enroll/revoke the Pi and iPhone, and authorize every operation against the owning account and SIM gateway.
- Maintain the device's outbound connection and expose HTTPS APIs and live WebSocket events to the app.
- Store durable SMS conversations, messages, segment outcomes, call records, command receipts, and ordered synchronization events in PostgreSQL.
- Queue outgoing SMS with an expiry and cancellation policy; retry transport safely using persisted idempotency keys.
- Coordinate call offers, acceptance, cancellation, timeouts, and state reconciliation. Persist history, but never queue a live call for later dialing.
- Send ordinary APNs notifications for messages and missed calls. Use PushKit VoIP notifications only for genuine incoming call invitations.
- Issue short-lived TURN credentials and relay encrypted WebRTC media. Start with cloud-relayed media for predictable routing; consider direct media only after reliability measurements.
- Monitor gateway presence, queue age, notification failures, and call setup/audio failures. Provide encrypted backups and tested restore procedures.

Proposed implementation: Python/FastAPI, PostgreSQL, a transactional outbox worker, and coturn, deployed as separate services on a maintained cloud host. A database-backed queue is enough for the initial single-device scale. TURN needs suitable public networking and bandwidth; it cannot be treated as an ordinary HTTP-only serverless endpoint. Choose hosting region through actual Pi-to-cloud and iPhone-to-cloud tests, including China.

The cloud terminates application TLS and can access SMS/history in the first design. Encrypt storage and backups and keep service keys separate. This is not a claim of end-to-end encrypted SMS. TURN forwards encrypted WebRTC packets without recording or decrypting audio; cellular voice outside the Pi/iPhone media connection has separate carrier security properties. TURN is needed because peer-to-peer connections are not always available. [WebRTC TURN guidance](https://webrtc.org/getting-started/turn-server)

### Proposed API surface

| Interface | Purpose |
|---|---|
| POST /v1/messages | Submit SMS with client-generated idempotency key |
| GET /v1/conversations; GET /v1/conversations/{id}/messages | Paginated message history |
| POST /v1/calls | Request an outgoing call; reject if gateway unavailable or busy |
| POST /v1/calls/{id}/actions | Accept, reject, hang up, or send DTMF with version checks |
| GET /v1/calls | Paginated call history |
| GET /v1/sync?cursor=... | Ordered changes including deletions and state updates |
| GET /v1/gateways/{id}/health | Presence, modem/SIM status, queues, and diagnostics |
| /v1/device/connect; /v1/app/events | Authenticated device channel and foreground app event stream |

Calls use authenticated signaling messages for SDP and ICE, scoped to the active call and enrolled endpoints. WebSocket transport acknowledgments are not proof that the modem executed an operation.

## 3. iOS app

Proposed stack: Swift/SwiftUI, a local persistent store, Keychain credentials, CallKit, PushKit/APNs, AVAudioSession, and a maintained WebRTC client library. iOS builds and real-device testing require macOS/Xcode; cloud Linux development can cover the device and cloud services.

### User-facing screens

| Screen | Required behavior |
|---|---|
| Messages | Conversations, unread counts, search, sender/recipient identity, timestamps |
| Conversation | Incoming/outgoing bubbles, compose/reply, Unicode, multipart reconstruction, pending/submitted/delivered/failed/unknown indicators |
| Calls | Incoming/outgoing/missed/rejected/failed history; number/contact, time, answered duration; tap to call back or message |
| Dialer | Number entry and explicit call through the remote SIM |
| Active call | Answer/reject/end, mute, speaker/Bluetooth routing, keypad/DTMF, elapsed connected time |
| Device/settings | Gateway/SIM status, sign-in, session revocation, notification preferences, history retention/deletion |

Use the app database for offline browsing. Persist unsent drafts locally; clearly distinguish drafts from accepted cloud requests. Calling requires live connectivity. Present the remote SIM identity so users understand which number and carrier originate the operation.

### Incoming-call integration

Register separate standard-push and VoIP-push tokens and update them when they rotate. For an incoming call, promptly report it through the applicable CallKit flow, establish signaling/media, and propagate user actions back to the device. Keep call UUIDs consistent across pushes, live events, and history to prevent duplicate rings. Resolve stale/cancelled invitations and end the system call UI when the actual call ends.

Follow Apple's current PushKit/CallKit requirements and validate on real devices in foreground, background, locked, OS-terminated, and user-force-quit states. Do not promise identical reachability in every lifecycle state. [Apple incoming VoIP guidance](https://developer.apple.com/documentation/pushkit/responding-to-voip-notifications-from-pushkit)

## Four end-to-end operations

### Receive SMS

Carrier delivers to modem -> Pi reads and durably stores -> Pi uploads event -> cloud persists and acknowledges -> cloud sends generic APNs notification -> app fetches and saves message -> conversation/unread state updates. If Internet is down, the Pi retains messages until synchronization. Modem storage capacity remains finite and must be monitored.

### Receive and answer a call

Modem reports incoming call -> Pi creates call ID and reports ringing -> cloud records offer with expiry and sends VoIP push -> iOS presents call UI -> user accepts -> cloud/device validate call is still ringing -> establish media readiness and answer the modem within the remaining carrier ring window -> bidirectional audio via TURN -> either side ends -> Pi confirms final modem state -> cloud/app reconcile one history record.

Prepare media while ringing where possible. If media cannot become ready before the caller stops ringing, show a failed/missed outcome, not a connected call. Duplicate accept requests must not create duplicate actions. If the caller hangs up during setup, propagate cancellation immediately.

### Send SMS

App creates message UUID -> cloud stores request and outbox entry -> Pi persists command -> modem submits segments -> device reports submission/delivery evidence -> cloud/app update the same message. When the gateway is offline, show queued state until expiry or cancellation wins before dispatch.

Exactly-once cellular delivery cannot be guaranteed across a crash after modem acceptance but before recording the response. Mark uncertain outcomes as unknown and reconcile available modem/carrier evidence; do not blindly retry an ambiguous send. Retrying such a message is a deliberate user action with a duplicate warning.

### Make a call

User dials -> cloud verifies gateway online and available -> Pi reserves call slot -> prepare WebRTC audio -> Pi dials via SIM -> cloud/app track dialing/ringing/connected -> connected time starts only on modem answer evidence -> hang up propagates both ways -> persist outcome and duration. Reject expired/replayed dial commands and concurrent second calls. Do not automatically redial after disconnection.

## History, consistency, and recovery

| Record | Minimum fields |
|---|---|
| Message | UUID, gateway/account, direction, raw and normalized address, body, encoding, segments, received/created times, status, failure/unknown reason |
| Conversation | ID, account/gateway, peer address, last message, unread/read watermark |
| Call | UUID, gateway/account, direction, remote number or withheld identity, offered/dialed/answered/ended times, outcome, cause, duration |
| Event | UUID, aggregate ID/version, origin, occurrence time, cloud sequence, payload version |
| Command | UUID/idempotency key, target, operation, expiry, acceptance/execution state, result |

Pi observations are authoritative for cellular state; cloud sequence governs synchronization ordering; app controls user read/deletion preferences. Preserve origin timestamps while using monotonic timers for live durations. Private/withheld caller IDs stay unknown rather than being invented or incorrectly grouped.

The cloud is the durable synchronized history store, the Pi is the durable intake/command journal, and iOS has an offline cache. Proposed initial retention: cloud/app history until user deletion, Pi payloads for 30 days after confirmed cloud acknowledgment, and no call audio recordings. Unsynced data is not silently expired; storage pressure generates an actionable error. Review storage limits before deployment.

Deletion creates a versioned tombstone so a reconnecting device cannot resurrect data. Purge active stores on synchronization and disclose backup expiration separately (proposed 30 days). A stale sync cursor requires a fresh snapshot. Replayed transport events update existing records, not duplicates. Raw network retries and identical SMS text are not interchangeable: legitimate repeated texts must remain distinct.

### Failure behavior

- Pi/cloud link lost: retain inbound SMS, queue only eligible outgoing SMS, reject new call requests, and expire incoming offers. The modem/carrier may continue ringing or apply carrier voicemail; the app must not claim it answered.
- Active call loses signaling/media: try bounded recovery (initial proposal: 10 seconds), then hang up the cellular leg to avoid an unattended charged call. Reconcile interrupted history after restart.
- Pi restarts: inspect actual modem call/storage state before accepting commands. Reconcile or terminate an orphaned call; never replay a persisted dial blindly.
- Push lost/delayed: app catches up from sync on reconnect. Notifications are hints, not the history database; an expired call must never ring as a new call.
- SMS arrives during voice: one serial owner handles both event types; verify carrier/device support in the acceptance suite.

## Security and operational requirements

Use separate revocable device and app identities, mutual TLS for Pi enrollment/connection, short-lived app sessions, and per-account authorization. Restrict TURN allocations with short-lived credentials. Store secrets outside Git; redact bodies, tokens, and full numbers from routine logs. Use encrypted persistent storage/backups with a documented boot-unlock/key-recovery strategy before unattended deployment; physical possession of an automatically unlocked Pi remains a risk.

Use systemd restart limits, a least-privileged device service, staged updates with rollback, and Tailscale for administrative access. Do not ship the development account's unrestricted passwordless sudo as an application capability. No call recording. Ordinary calls originate physically at the remote SIM; emergency dialing is outside this app's supported use and must be blocked for configured destinations, directing the user to the local phone instead.

## Review decisions

Proposed defaults: one account/SIM/active call, cloud-held history with app cache, all voice media through TURN initially, Python services and native SwiftUI app. Items requiring evidence: modem audio path, China Mobile voice/roaming compatibility, cloud region, real-device notification behavior, backup-power equipment compatible with Pi 5, and acceptable measured call latency.

Approve or adjust these defaults through the design PR. No production hosting purchases, actual SMS/calls, device configuration changes, or application deployment are performed by this documentation change.
