# Implementation Plan — SMS and Calls

Review companion to [the three-component design](SMS_GATEWAY_DESIGN.md). Every milestone is pending unless explicitly marked as observed hardware setup. The release requires receiving SMS, receiving calls, sending SMS, making calls, and synchronized histories.

## Implementation breakdown by component

| Work package | Device proxy | Cloud proxy | iOS app |
|---|---|---|---|
| Foundation | Single modem owner; parser; SQLite journal; systemd | Enrollment, authentication, PostgreSQL, API schemas | App shell, sign-in, local store, secure credentials |
| Receive SMS | Read/decode/store; multipart handling; storage reconciliation | Durable ingest, deduplication, sync, APNs | Inbox/conversations, unread state, notification handling |
| Send SMS | Durable commands; segment submission; delivery/unknown outcomes | Idempotent requests, queue/expiry/cancel | Compose/reply, status, explicit retry |
| Receive calls | Ring/caller ID, answer/reject/end, audio bridge | Expiring offers, PushKit provider, signaling, TURN | CallKit incoming UI, answer/reject, live audio |
| Make calls | Dial/state/DTMF, one-call lock, audio bridge | Availability check, call commands, signaling | Dialer, outgoing UI, mute/routes/keypad/end |
| History | Recoverable message and call event journal | Canonical records, cursor sync, deletion tombstones, backups | Offline SMS/call history, search, callbacks, reconciliation |
| Operations | Health, restart recovery, bounded watchdog, updates | Monitoring, authorization, notification retries, restore | Gateway health, lifecycle handling, session revoke |

## Milestones and reviewable PRs

### M0 — Prove SMS, cellular voice, and audio hardware

Already observed: Pi/Tailscale access, fan operation, modem control, SIM READY, and LTE roaming registration. These are setup evidence, not end-to-end acceptance.

- PR 1: save reusable modem diagnostics and a hardware runbook, without committing identifiers, secrets, or message contents.
- Exercise inbound/outbound SMS with known test participants, including Chinese text and multipart messages.
- Exercise incoming/outgoing cellular calls and DTMF on the exact SIM/modem firmware; confirm actual two-way audio.
- Prove simultaneous modem audio capture/playback from Linux. Document selected USB/PCM/analog approach and any required hardware changes.
- Repeat after a power cycle and record sanitized results separately for US roaming and eventual China deployment.

Exit: three SMS round trips and three answered calls in each direction; one 10-minute bidirectional audio session through the Pi without unexplained audio loss. Actual calls/messages require designated test numbers and explicit test authorization. A failed audio gate blocks voice implementation choices, not the voice requirement; propose remediation before proceeding.

### M1 — Shared contracts and service skeletons

Depends on design review; may proceed alongside M0 using a simulated modem.

- PR 2: versioned commands/events, SMS/call state machines, mock scenarios, API schema, and migration rules.
- PR 3: device service, serial parser/arbiter, SQLite outbox, modem simulator, and systemd packaging.
- PR 4: cloud API, PostgreSQL schema/outbox, device/app enrollment and authorization, development deployment.
- PR 5: iOS project, local database, sign-in, networking, and Messages/Calls/Device navigation.

Exit: simulator events synchronize device -> cloud -> app; replayed events and unauthorized requests have correct outcomes. CI runs device/cloud tests; an available macOS runner or developer Mac builds the iOS target.

### M2 — Receive and send SMS end to end

Depends on M1 and the SMS portion of M0.

- PR 6: inbound PDU decoding/reassembly, durable ingestion, conversation history, normal APNs, and iOS inbox.
- PR 7: outgoing compose/reply, cloud queue/expiry, modem submission, per-segment status, delivery reports, and explicit handling of uncertain sends.
- PR 8: offline history, unread synchronization, search, deletion tombstones, and restore/snapshot handling.

Exit: independent phone -> SIM -> Pi -> cloud -> iOS and reverse both pass. Test ASCII, Chinese, emoji, and multipart texts; 20 controlled round trips across several sessions. Exercise modem/cloud/Pi restart at submission and acknowledgment boundaries. No duplicate caused by replaying the same application request; an ambiguous carrier submission remains visibly unknown rather than being blindly resent.

SMS is an intermediate deliverable, not overall completion.

### M3 — Make voice calls through the full system

Depends on M1 and full M0 audio proof. Voice audio worker and TURN deployment can proceed alongside M2 after those dependencies pass.

- PR 9: device audio worker, WebRTC sessions, secure TURN deployment, credential issuance, and media diagnostics.
- PR 10: outgoing call coordination, modem call state, iOS CallKit/audio session, dialer, mute, speaker/Bluetooth, DTMF, and hang up.
- Persist outgoing history from real state transitions, including busy, rejected, no answer, failed setup, and interrupted calls.

Exit: iPhone on a separate network calls an independent phone through the SIM; both parties hear each other for 10 minutes. Test hang-up from either endpoint, keypad tones, second-call rejection, packet loss, and disconnected signaling. Carrier caller-ID presentation is observed, not assumed. No automatic redial.

### M4 — Receive voice calls with background iOS handling

Depends on M3 and physical iPhone provisioning for push and CallKit.

- PR 11: inbound modem events, expiring cloud offers, PushKit provider, token rotation, CallKit incoming calls, and device answer/reject.
- PR 12: cancellation races, duplicate/late pushes, timeouts, missed/rejected/answered records, app restart reconciliation, and call-back actions.

Exit: incoming call -> iPhone alert -> answer -> two-way audio works while the app is foregrounded, backgrounded, and the phone locked. Test OS termination and explicit force-quit separately and document actual platform behavior. Verify caller cancellation before answer, rejection, busy gateway, unavailable phone, and network failure. Push acceptance alone does not count as ringing or answer success.

### M5 — History, security, and unattended reliability

Depends on M2 and M4.

- PR 13: unify call/SMS history sync, retention, deletion, encrypted backups, and restore to a fresh app/device.
- PR 14: session/device revocation, tenant-isolation tests, restricted service permissions, secret management, redacted diagnostics, and notification privacy.
- PR 15: reconnect/restart recovery, bounded orphan-call cleanup, monitoring, update/rollback, and installation/recovery guide.

Exit: all four operations and histories pass across Pi reboot, modem unplug, cloud restart, router/network outage, app reinstall, and backup restore. Test SMS during a call. No permanently stuck call UI, unexpected re-dial, or lost acknowledged application history. Detect missing or uncertain evidence explicitly.

### M6 — China deployment acceptance

Depends on M5. Measure connectivity before selecting final cloud/TURN region; no specific network route is assumed reliable.

- PR 16: deployment configuration, verified Pi 5 power/cooling arrangement, local recovery instructions, and sanitized acceptance report.
- Re-run all four operations on the final China Mobile network with the iPhone outside the gateway LAN.
- Run a 30-day soak with agreed scheduled test calls/messages, monitoring, failure injection, and backup restoration. Agree test recipients and charges before enabling automated cellular tests.

Exit: all release criteria below pass; no unexplained lost acknowledged records or unattended live calls, and a documented local recovery path is available.

## Release acceptance checklist

| Required goal | Acceptance evidence |
|---|---|
| Receive SMS | External phone sends to SIM; one complete message reaches iOS history with correct sender/content; offline recovery verified |
| Receive calls | External phone calls SIM; iOS rings and answers; two-way audio and remote hang-up work; answered/missed/rejected history correct |
| Send SMS | iOS submits through SIM; independent receiver gets the text; history accurately distinguishes submission, confirmed delivery, failure, and uncertainty |
| Make calls | iOS dials via SIM; independent phone rings; bidirectional audio, DTMF, routing, and hang-up work; duration/outcome recorded |
| Durable history | SMS threads and call records survive app relaunch, reconnect, reinstall/resync, and cloud restore; deletions do not reappear |

Proposed measurable targets for review: device-to-app SMS synchronization within 10 seconds when both are online (carrier transit measured separately); incoming call presentation within 5 seconds of the device's ring event on supported app states; media established within 3 seconds of answer under the measured test network; one-way audio latency target below 400 ms on the deployment route. These are engineering targets, not current guarantees. Report measured percentiles, failed attempts, and route conditions before accepting or revising targets.

## Repository and development workflow

Proposed directories: `device/`, `cloud/`, `ios/`, `contracts/`, `tests/`, `deploy/`, and `docs/`. Create them as their implementations begin; no empty scaffolding is required by this plan.

Use a separate descriptive `codex/` branch for each logical change and open a PR against main. The user reviews and merges; no direct main pushes or auto-merge. Maintain contract compatibility during staggered device/cloud/app updates. Cloud development uses the modem simulator for routine tests and Tailscale SSH for explicit hardware integration runs. CI must not dial or send SMS automatically.

## Decisions to review before implementation

1. Confirm one SIM and one active call for the first release.
2. Confirm cloud-held message/call history plus an offline iOS copy; end-to-end encrypted message storage would require a separate key-management design.
3. Confirm familiar Phone/Messages-style screens within this app, with CallKit for live calls.
4. Review initial retention and latency targets after the hardware/network proof.
5. Select cloud hosting/region and the Pi audio path based on evidence. Calendar estimates follow M0 because voice feasibility and iOS lifecycle behavior are the largest unknowns.
