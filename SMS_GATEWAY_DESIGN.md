# China Physical-SIM Remote SMS Gateway

**Status:** purchase-and-build design, SMS only  
**Primary outcome:** the Chinese physical SIM stays in a powered device in China while a dedicated iPhone app securely sends and receives its SMS messages from the US.

## 1. Scope and constraints

### In scope

- Receive standard SMS addressed to the Chinese mobile number.
- Send standard SMS that originates from that number.
- Notify the iPhone promptly and show a durable conversation history in a dedicated app.
- Run unattended in a China home using Ethernet as primary connectivity and Wi-Fi as backup.
- Gracefully ride through short power cuts, recover from modem/network faults, and support remote diagnostics.

### Explicitly out of PoC scope

- Voice calls, MMS, RCS, cellular data routing, SIM cloning, or eSIM conversion. Voice is documented later only as a possible Phase 2 stretch goal; no voice software or voice reliability claim is part of the SMS PoC acceptance criteria.
- Integration with Apple Messages. iOS will not treat a SIM that is physically in a remote modem as a local carrier line. The iPhone therefore uses a dedicated companion app.
- Exposing SSH, the modem, or a web dashboard directly to the public Internet.

### Fixed carrier assumption

This design is for an existing **China Mobile** physical SIM only. Supporting China Unicom or China Telecom is not a requirement, though the recommended global modem may happen to work with them. You have confirmed there is no restriction on registering this SIM in an LTE modem.

The SIM is assumed to have no SIM PIN. Before purchase, confirm only operational details: whether international roaming is active for US development, the plan’s SMS/roaming charges, and whether any account-side anti-fraud control might interrupt unattended use.

## 2. Proposed system

```text
Chinese SIM → LTE modem → Raspberry Pi gateway ──outbound mTLS/WebSocket── cloud relay
                                      │                                      │
                              Ethernet / Wi-Fi                         APNs push
                                                                             │
                                                                    iPhone app
```

The Pi owns the modem. It records inbound SMS locally before acknowledging it upstream. It maintains one outbound encrypted connection to a small relay service. The relay delivers push notifications and queues outgoing messages while the home gateway is disconnected. The iPhone app authenticates as a separate client, reads its messages from the relay, and asks the gateway to send messages.

The physical SIM never leaves the gateway after installation. The service must use no cellular data plan; home Ethernet/Wi-Fi provides its Internet access.

## 3. BOM — recommended development-to-deployment build

The following is the exact PoC order list. Each link goes to the manufacturer or an established electronics retailer; stock and price were checked on **September 19, 2026** but can change. Do not substitute a region-specific modem merely because its name is similar.

| Qty | Required? | Exact item and purchase link | Exact model / specification | Purpose and notes |
|---:|---|---|---|---|
| 1 | Yes | [Raspberry Pi 4 Model B — 4 GB RAM, SparkFun](https://www.sparkfun.com/raspberry-pi-4-model-b-4-gb.html) | **Raspberry Pi 4 Model B, 4 GB**; Broadcom BCM2711 quad-core 64-bit Cortex-A72 at 1.8 GHz; 4 GB LPDDR4-3200; dual-band 802.11ac Wi-Fi; Bluetooth 5/BLE; gigabit Ethernet; 2× USB 3, 2× USB 2; microSD; 40-pin GPIO; 5 V/3 A USB-C input | This is the exact board variant to order. The 2 GB model would run the service, but 4 GB provides comfortable build, logging, database, and future voice headroom without the cost of 8 GB. |
| 1 | Yes | [Waveshare SIM7600G-H 4G HAT, SKU 17372](https://www.waveshare.com/product/iot-communication/sim7600g-h-4g-hat.htm) | **SIM7600G-H global-band LTE Cat-4 HAT**; LTE-FDD B1/2/3/4/5/7/8/12/13/18/19/20/25/26/28/66; LTE-TDD B34/38/39/40/41; nano-SIM; USB/UART AT interface; SMS text/PDU; audio jack and decoder | Correct modem for US roaming development plus China Mobile deployment. The package lists one LTE antenna, one GNSS antenna, two USB-A-to-micro-B cables, and mounting screws. Voice-capable hardware is preserved for the post-PoC stretch goal. |
| 2 | Yes | [SanDisk 64 GB High Endurance microSDXC, SDSQQNR-064G-GN6IA, B&H](https://www.bhphotovideo.com/c/product/1987067-REG/sandisk_sdsqqnr_064g_gn6ia_64gb_high_endurance_microsdhc.html/overview) | 64 GB, UHS-I, U3/V30/Class 10, up to 100 MB/s read and 40 MB/s write | One active card and one pre-flashed encrypted recovery spare. Buying from an authorized dealer reduces counterfeit-media risk. |
| 1 | Yes | [Official Raspberry Pi 15 W USB-C Power Supply — US, Waveshare](https://www.waveshare.com/pi-psu-us-w.htm) | Official Pi 4 supply; 5.1 V/3 A output; **100–240 V AC, 50/60 Hz input** | Works electrically in both the US and China. A physical plug adapter may be needed for a Type-I-only China outlet. |
| 1 | Yes | [PiSugar 3 Plus 5000 mAh UPS](https://www.pisugar.com/products/pisugar-3-plus-raspberry-pi-ups) | Integrated 5000 mAh battery; 5 V/3 A maximum input and output; Pi 4 compatible; I²C battery/RTC monitoring | PoC backup power and software-visible battery state. Its 3 A ceiling is close to the combined worst case, so sustained modem transmit and mains-transfer tests are mandatory before China deployment. |
| 1 | Yes | [Large 40 × 30 × 5 mm Raspberry Pi 4 heatsink, PiShop.us](https://www.pishop.us/product/large-heatsink-for-raspberry-pi-4-random-color/) | Low-profile anodized aluminum, adhesive thermal pad, Pi 4 compatible | Passive cooling that fits under most HATs. Temperature must still be monitored during the 30-day burn-in. |
| 1 | Yes | [Black nylon M2.5 screw and standoff kit, Adafruit product 3299](https://www.adafruit.com/product/3299) | Assorted M2.5 screws, nuts, and 6–12 mm insulating standoffs | Provides electrically safe mounting options for the Pi, UPS, modem, and enclosure. |
| 1 | Yes | [Monoprice 5 ft Cat6 patch cable, product 3427](https://www.monoprice.com/Product?p_id=3427) | 24 AWG stranded pure-copper UTP, RJ45, 550 MHz | Ethernet is the primary home-network connection. Choose a longer length on the same product page if the router location requires it. |
| 1 | Yes | [Ceptics IG-16 grounded US-to-China Type-I adapter](https://www.ceptics.com/products/australia-china-travel-adapter-type-i-industrial-grade-ig-16) | NEMA 5-15 input to China/Australia Type I; ETL listed; up to 250 V; **does not convert voltage** | Allows the universal-input official Pi supply to fit a Type-I China outlet. Many China outlets accept US Type A directly, but do not rely on that for an unattended installation. |
| 1 | Yes | [Hammond 1598GBK ABS enclosure, DigiKey HM168-ND](https://www.digikey.com/en/products/detail/hammond-manufacturing/1598GBK/131059) | 249.7 × 159 × 75 mm; flame-retardant ABS; removable end panels; IP54 | Large enough to prototype the complete stack and create connector/vent cutouts. Do not close it permanently until thermal and antenna tests pass. Antennas stay outside. |
| 1 | Yes | [USB microSD reader/writer, Adafruit product 939](https://www.adafruit.com/product/939) | USB-A reader for microSD/microSDHC/microSDXC | Used to flash, verify, clone, and restore both microSD cards. Skip only if a trusted reader is already available. |
| 1 | Spare | [Adafruit USB-A-to-micro-B data cable, product 592](https://www.adafruit.com/product/592) | USB 2 data cable, approximately 1 m | The modem package already includes two; this is an inexpensive known-data-capable spare for recovery/debugging. |
| 1 | Optional reception upgrade | [Waveshare 4G high-gain SMA antenna, SKU 21121](https://www.waveshare.com/product/4g-3g-2g-lpwa-external-antenna.htm) | 698–960 / 1710–2690 MHz, 4 dBi, 50 Ω, SMA male | Buy one now only if you want a spare. Decide on an outdoor/magnetic antenna after measuring signal at the actual China installation site. |
| 1 | Optional voice provisioning | [Apple EarPods with 3.5 mm plug](https://www.apple.com/shop/product/mwu53am/a/earpods-35mm-headphone-plug) | CTIA-style headset with microphone | Not used by the SMS PoC. It permits a direct local call/VoLTE check through the modem audio jack before any future voice software is built. |

**Expected hardware spend:** approximately **US$320–380** before shipping/tax, depending on Pi availability and whether the optional/spare items are ordered. Ongoing cloud hosting and Apple Developer Program costs are separate.

### Exact Raspberry Pi selection

Order **Raspberry Pi 4 Model B with 4 GB RAM**, not Pi 5, Pi 400, Compute Module 4, or a 2/8 GB substitution. The exact board gives this PoC:

- enough CPU/RAM for the gateway daemon, SQLite, secure tunnel, monitoring, and development tools;
- onboard gigabit Ethernet and dual-band Wi-Fi without adapters;
- four USB host ports for the modem, recovery, and a possible future USB-audio fallback;
- lower and simpler power requirements than Pi 5; and
- a mature Raspberry Pi OS and accessory ecosystem.

### Power wiring recommendation

For the first build, mount the PiSugar below the Pi and the SIM7600 HAT above it, while using the modem’s supplied USB cable for its data interface. The official USB-C supply feeds the UPS, and the UPS feeds the Pi/HAT stack. Do not bypass the UPS by powering the Pi’s USB-C port simultaneously unless the PiSugar instructions explicitly call for that configuration.

The PiSugar’s specified 5 V/3 A output is a **PoC candidate, not yet a deployment guarantee**. The SIM7600 hardware guide says its internal supply must not fall below 3.4 V during cellular bursts that can approach 2 A. The complete assembly must therefore prove it can tolerate repeated SMS transmission, weak-signal transmission, mains removal, recharge, and reboot without undervoltage or modem resets.

Use a bench test before enclosure assembly: Pi + UPS + modem + antennas, then repeatedly send SMS while unplugging and restoring mains. If the selected UPS cannot maintain a stable Pi supply with the modem present, stop using it for deployment and move to a tested 5 V/5 A-class UPS architecture. Do not hide a marginal power system by disabling undervoltage reporting.

## 4. Hardware alternatives

| Option | When to choose it | Tradeoff |
|---|---|---|
| **A. [Pi 4](https://www.sparkfun.com/raspberry-pi-4-model-b-4-gb.html) + [SIM7600G-H HAT](https://www.waveshare.com/product/iot-communication/sim7600g-h-4g-hat.htm) (recommended)** | You want one build to develop in the US and deploy in China. | Larger and more hands-on than an appliance, but easiest to debug. |
| **B. Pi 4/CM4 + SIM7600G-H M.2 HAT** | You value stronger physical mounting, external module power, and a cleaner final enclosure. | More mechanical work; buy only after Option A proves the SIM/carrier behavior. [Board](https://www.waveshare.com/product/sim7600g-h-m2-4g-hat.htm) |
| **C. Pi 4 + [SIM7600CE China-specific HAT](https://www.waveshare.com/sim7600ce-4g-hat.htm)** | US roaming is unimportant and China Mobile-only deployment is the priority. | Better China-centric band set, but much weaker as a US test platform; not recommended for the first purchase. |
| **D. Keep the iPhone 13 as the gateway temporarily** | You need near-zero development risk while building the real appliance. | It solves the practical problem but does not meet the desired embedded-device goal. Use only as fall-back during the China cutover. |

Do not buy a 2G-only SIM800/GoIP-style gateway. Its network longevity and voice/SMS behavior are poor assumptions for an unattended deployment.

## 5. Software design

### Components

1. **Gateway daemon (Pi; Python or Go).** Owns the modem serial port, modem state machine, SQLite database, health checks, and the outbound relay connection.
2. **Cloud relay.** Small HTTPS/WebSocket service with PostgreSQL. It persists message envelopes, authenticates the Pi and iPhone separately, and sends Apple Push Notification service (APNs) notifications.
3. **iPhone app (Swift/SwiftUI).** Conversation UI, send/retry state, notification handling, device-health screen, and biometric lock. TestFlight distribution is sufficient for a personal build.
4. **Operations plane.** A private overlay VPN or a mutually authenticated outbound tunnel for emergency SSH and dashboards. No open inbound ports at the China home.

### Gateway responsibilities

- Detect modem availability; issue `AT`, query SIM readiness/network registration/signal, and set SMS text mode.
- Receive unsolicited SMS notifications, fetch full content, normalize telephone numbers to E.164 where possible, store locally, then sync upstream.
- Allocate a UUID to every inbound and outbound message. At-least-once transport is acceptable only because UUID-based de-duplication is enforced at the relay and app.
- Persist an outbound request before sending it to the modem. Record modem acceptance and final delivery report separately.
- Exponentially back off for carrier or relay failures. Restart the modem after a bounded number of failed registration attempts; restart the daemon automatically with `systemd`.
- Retain message content locally for a defined period (for example 90 days), then purge it; backups should be encrypted.

### Security model

- Unique device key in the Pi, separate app-user key/session, TLS everywhere, and mutual TLS between Pi and relay.
- Encrypt the local database and backups. Do not write message bodies, credentials, API tokens, or full phone numbers to journal logs.
- Require Face ID/passcode to open the app. Support a remote revoke switch that disables an unrecognized phone session.
- Treat incoming SMS content as untrusted text; never execute URLs or commands from it.

## 6. Implementation milestones

### M0 — carrier and hardware proof (one afternoon)

1. Verify the SIM can currently send/receive an SMS in the iPhone 13.
2. Insert it into the unpowered modem, attach antennas, then power the Pi and modem.
3. Install Raspberry Pi OS Lite 64-bit, enable SSH with key-only authentication, set a unique hostname, patch the OS, and connect Ethernet.
4. Confirm modem USB ports appear and prove the minimal AT commands: modem identity, SIM-ready status, network registration, signal strength, inbound SMS, outbound SMS.
5. Record the modem firmware version, IMEI, carrier, RSSI/RSRP, and test timestamp in the deployment record.

**Exit criterion:** three successful send/receive SMS cycles with the SIM in the modem, after one complete reboot.

### M1 — local gateway service (2–4 days)

1. Create a small service with a fake-modem interface and automated unit tests before attaching production SIM state.
2. Implement serial command framing, timeouts, unsolicited-event parsing, and a modem state machine.
3. Create SQLite tables for messages, delivery events, modem state, and idempotency keys.
4. Expose a localhost-only REST API: health, list messages, submit outgoing message, and message status.
5. Add a `systemd` service, watchdog, structured redacted logs, and a local health endpoint.

**Exit criterion:** a laptop on the home LAN can send/receive SMS through the localhost gateway API; duplicate API requests never produce duplicate messages.

### M2 — secure remote relay (2–4 days)

1. Deploy a minimal relay on a maintained server with a domain, TLS certificate, database backups, and monitoring.
2. Enroll the gateway with a one-time provisioning token, then replace it with a device certificate/key.
3. Implement durable queues and idempotency on both directions: Pi→relay for inbound SMS and relay→Pi for outbound SMS.
4. Add health telemetry: last modem registration, last relay contact, storage free space, signal, and battery/mains state if available.
5. Test the gateway while home Internet is absent, then restored.

**Exit criterion:** no inbound message is lost during a simulated relay outage, and every outbound request is either delivered once or visibly fails with a reason.

### M3 — iPhone app and push (3–6 days)

1. Create a SwiftUI app with sign-in/enrollment, conversation list, message detail, compose/send, and device-health view.
2. Register for APNs; send a push that contains no SMS body—only a message identifier. Fetch content over authenticated TLS after the user opens/foregrounds the app.
3. Implement local notification display, reconnect/retry behavior, outgoing pending/sent/failed state, and biometric app lock.
4. Distribute through TestFlight to the iPhone 17.

**Exit criterion:** an incoming SMS produces a prompt iPhone notification and appears exactly once in the app; an app-sent SMS arrives on an independent test phone with the Chinese SIM number as sender.

### M4 — hardening and China deployment rehearsal (2–4 weeks)

1. Run it continuously in the US on the Chinese SIM’s roaming service, with automated daily test messages.
2. Perform controlled mains loss, Ethernet loss, Wi-Fi loss, router reboot, Pi reboot, modem reboot, and relay restart tests.
3. Confirm encrypted backup restore onto the spare microSD card.
4. Package a deployment kit: primary device, spare storage, antennas, power adapter, carrier details, recovery guide, and iPhone 13 fall-back.

**Exit criterion:** 30 consecutive days without an unexplained lost test SMS; all forced faults recover without local intervention.

## 7. Test strategy

### Test participants

- **Gateway number:** the Chinese SIM in the modem.
- **Control phone A:** iPhone 13 or another phone that can receive/send independent SMS.
- **Control phone B:** a US number on a different carrier, if available.
- **Client:** iPhone 17 running the TestFlight app.

### Test matrix

| Test | Procedure | Pass condition |
|---|---|---|
| Inbound China-number SMS | A sends ASCII, Chinese, emoji, and long multipart messages to the gateway SIM. | One app conversation/message per original SMS; timestamp, sender, and Unicode body preserved. |
| Inbound US-number SMS | B sends messages to the gateway SIM while it roams in the US. | Same as above, or the carrier’s documented roaming limitation is recorded. |
| Outbound to China | Send from the iPhone app to A. | A receives one message, with the Chinese gateway number as sender. |
| Outbound to US | Send from the iPhone app to B. | B receives one message or a carrier failure is accurately reported. |
| Multipart/Unicode | Send 3+ segment Chinese and emoji texts each direction. | Correct ordering and no duplicated/corrupted segments. |
| Offline relay | Disconnect home Internet, send inbound SMS, reconnect. | Gateway stores it and app receives it after reconnect. |
| Offline gateway | Stop the daemon or disconnect Pi network, then restore. | Relay queues outgoing request once; sends exactly once after recovery. |
| Power loss | Remove mains for 5, 30, and 60 minutes, restore. | UPS remains stable for selected target; Pi/modem register and reconnect automatically. |
| Modem recovery | Power-cycle modem or simulate registration loss. | Service detects failure, recovers within the defined timeout, and exposes status. |
| Security | Attempt API use with invalid/expired app token and inspect logs/backups. | Access denied; no SMS content or secrets exposed. |

### E2E iPhone acceptance test

1. Put the iPhone 17 on cellular data—not the same Wi-Fi as the gateway.
2. From control phone A, send `E2E-IN-<timestamp>` to the Chinese SIM. Confirm a push notification and one matching message in the app within the chosen service target (start with 60 seconds).
3. Reply from the iPhone app with `E2E-OUT-<timestamp>`. Confirm A receives it once, from the Chinese number.
4. Put the iPhone in the background/locked state and repeat step 2. Confirm the notification still arrives; then open the app and verify the full message is retrieved.
5. Disable the gateway Internet for five minutes; send another inbound SMS. Restore Internet and verify delayed but complete delivery. Then send an outgoing app message while the gateway is offline and verify it queues and sends once on recovery.
6. Export the test run: message UUIDs, times, status transitions, modem registration state, signal, gateway version, and relay version—without exporting message bodies to general logs.

## 8. Known risks and mitigations

| Risk | Mitigation |
|---|---|
| Unexpected roaming or account-side SMS limits | Modem registration is approved for this China Mobile SIM. Validate only real-world US roaming, billing, and SMS behavior during M0; retain the iPhone 13 for China cutover. |
| US roaming does not represent China behavior | Treat US as hardware/software validation only. Re-run the full acceptance suite after China installation. |
| Cross-border remote connectivity is unreliable | Use Ethernet primary, outbound-only connections, queues, and monitoring. Do not assume any particular VPN is permanently reachable from China. |
| Power/thermal instability | Use certified power equipment, test transmit bursts and power cuts, ventilate enclosure, and monitor modem resets. |
| SMS privacy/2FA exposure | End-to-end authenticated transport, encrypted storage, minimal logs, app biometric lock, key/session revocation. |

## 9. Stretch goals and nice-to-haves

These are deliberately not requirements for the first SMS release. Add them only after the base system has passed the end-to-end and 30-day reliability tests.

### Operations and monitoring

| Feature | What it does | Why it is valuable |
|---|---|---|
| **Device health card in the iPhone app** | Shows online/offline state, last heartbeat, uptime, software version, modem registration, carrier, signal, storage free space, temperature, Ethernet/Wi-Fi state, and battery/UPS charge. | Answers “is it alive?” without SSH or a local helper. |
| **Push alerts with severity** | Immediate alerts for mains loss, battery low, gateway offline, SIM removed/locked, modem unregistered, repeated send failure, full storage, or failed backup. Informational alerts for recovery. | The device should tell you about a problem rather than silently missing an important text. |
| **Heartbeat watchdog** | Gateway emits a signed heartbeat every 1–5 minutes. The relay alerts if several are missed, even if the Pi cannot send its own failure report. | Detects total power, Internet, OS, or application failure. |
| **Power-event timeline** | Records mains-loss time, battery percentage, shutdown, restore time, and recovery duration. | Makes it obvious whether outages, insufficient UPS capacity, or software recovery are the problem. |
| **Weekly reliability digest** | A quiet weekly summary: uptime, number of messages, delivery latency percentiles, resets, low-signal periods, backup outcome, and pending software updates. | Avoids alert fatigue but preserves visibility. |
| **Synthetic health check** | Daily test of modem AT responsiveness, SIM readiness, relay connection, database integrity, and APNs reachability; optionally a monthly real SMS to a designated control number. | Detects partial failure before an important OTP is missed. Use real SMS sparingly to avoid charges or confusing recipients. |

### Automatic recovery

| Feature | Behavior | Guardrail |
|---|---|---|
| **Escalating self-heal** | Reconnect relay → restart daemon → reset modem → reboot Pi, only as each earlier step fails. | Rate-limit each action and preserve diagnostics before rebooting, so a loop does not hide the root cause. |
| **Remote modem power cycle** | Controls the modem’s power/reset pin or a dedicated USB power switch. | Require authenticated user action except for a clearly defined automatic threshold. |
| **Network failover** | Prefer Ethernet; fall back to stored Wi-Fi; optionally use the modem’s cellular data only for emergency telemetry. | Keep data fallback disabled by default until plan charges and carrier support are verified. |
| **Safe remote update / rollback** | Download a signed release, install in a maintenance window, confirm healthy heartbeat, and automatically roll back after a failed boot or failed health check. | Never remotely update the OS and application in one uncontrolled operation. Keep the current image and the spare microSD. |
| **Config and secret backup** | Encrypted backup of database, configuration, certificates, and enrollment state; independently verifies restore. | Back up keys safely; never make them recoverable from an ordinary email attachment. |

### SMS experience improvements

| Feature | What it does | Important limitation |
|---|---|---|
| **Delivery state and audit trail** | Shows queued, accepted by gateway, submitted to carrier, delivery report if available, and failed/retry reason. | Carrier delivery reports are not uniformly available; do not display “delivered” unless it is actually confirmed. |
| **Sender labels and search** | Contacts, conversation search, Chinese/English full-text search, and tags such as bank, family, work, or verification code. | Keep indexed content encrypted at rest. |
| **OTP focus mode** | Detects likely verification-code messages locally and presents a high-priority notification with a one-tap copy action. | Make this opt-in; do not forward codes to email or third-party automation. |
| **Rules and quiet hours** | Silence low-priority senders overnight, flag unknown senders, or notify a trusted second device only for explicitly chosen contacts. | Avoid automatic replies; they can expose that the number is remotely operated. |
| **Attachment-safe export** | Encrypted, user-initiated export of selected conversations for personal archival. | SMS is plain text and may contain sensitive credentials; exclude it from ordinary device backups by default. |

### Security and administration

| Feature | What it does | Why it matters |
|---|---|---|
| **SIM identity tamper alert** | Pins expected ICCID/IMSI during enrollment and alerts if another SIM appears. | Detects accidental SIM swaps or physical tampering. |
| **Session/device management** | Lists enrolled iPhones, their last access, and supports instant revoke. | Lets you retire a lost iPhone without replacing the SIM. |
| **Recovery codes and break-glass path** | Offline recovery codes permit re-enrollment when the iPhone is lost. | Store separately from the device and never in unencrypted notes. |
| **Remote-support bundle** | Generates a redacted, encrypted diagnostics package: state transitions, software versions, signal trend, and error codes—no message content. | Greatly reduces the need to expose live remote shell access. |
| **Maintenance calendar** | Reminder before certificate expiry, OS support end, backup failure, or UPS battery replacement interval. | Turns predictable degradation into scheduled maintenance. |

### Physical/deployment enhancements

- Add a small monitored smart plug **only as a secondary signal**; it cannot prove the Pi is healthy and it must not be the only recovery mechanism.
- Add a cellular signal/temperature chart. It helps decide whether moving the antenna, not rewriting software, will fix missed messages.
- Add a tamper-evident enclosure seal and a printed local recovery card for a trusted person in China.
- Keep the device on a separate home-network VLAN/guest segment if the router supports it, while still allowing its outbound relay/VPN connection.

### Phase 2 stretch goal — remote voice calls (**not part of the PoC scope**)

The selected SIM7600G-H preserves a possible future voice path: Waveshare documents telephone call control, DTMF, an onboard audio decoder, and a 3.5 mm audio jack; SIMCom also publishes a USB-audio application note. This is why the BOM keeps the voice-capable SIM7600G-H instead of optimizing for an SMS-only modem.

Voice is nevertheless a separate real-time communications project. The SMS PoC is complete without any of the following software, services, tests, or guarantees.

```text
China Mobile caller
        │ cellular voice / VoLTE
        ▼
SIM7600G-H in China ── live PCM audio ── Pi voice gateway
                                                │ encrypted WebRTC
                                                ▼
                                      signaling + TURN relay
                                                │
                                                ▼
                                      iPhone app + CallKit
```

#### BOM impact

- **Required now:** no additional production component. The Pi 4 has sufficient headroom and USB ports, and the selected modem already supports call control and audio.
- **Recommended now for a cheap feasibility test:** the optional [Apple 3.5 mm EarPods](https://www.apple.com/shop/product/mwu53am/a/earpods-35mm-headphone-plug) listed in the BOM. They allow direct modem calling tests before building any VoIP bridge.
- **Buy later only if USB audio is not usable:** a Linux class-compliant adapter such as the [Sabrent AU-MMSA USB audio adapter](https://sabrent.com/products/au-mmsa). Its analog input/output must not be connected directly to the modem with an improvised cable until audio levels, CTIA pinout, attenuation, and isolation are designed and verified.
- **Cloud addition:** a TURN media relay is normally required because direct WebRTC connectivity through two NATs is not dependable. [WebRTC explains the TURN requirement](https://webrtc.org/getting-started/turn-server).

#### Additional software

1. **Modem call controller:** ringing/caller ID, dial, answer, reject, busy, hang-up, DTMF, timeouts, and a single-call state machine.
2. **Pi audio bridge:** capture modem PCM audio, encode/decode Opus, send/receive WebRTC media, and monitor one-way audio, jitter, loss, gain, and mute state.
3. **Relay signaling and TURN:** short-lived call invitations, WebRTC negotiation, NAT traversal, hang-up propagation, and call heartbeat.
4. **iPhone calling layer:** WebRTC audio plus PushKit and CallKit. CallKit supplies the native-looking system call interface, but the Chinese SIM is still remote and is not installed as an iPhone cellular line. Apple requires VoIP pushes to be promptly reported to CallKit. [Apple CallKit](https://developer.apple.com/documentation/CallKit), [Apple VoIP push handling](https://developer.apple.com/documentation/pushkit/responding-to-voip-notifications-from-pushkit).
5. **Operations and security:** call activity alerts, authentication, rate/duration limits, session revocation, media metrics, and no recording by default.

#### Phase 2 validation gates

1. **V0 — China Mobile voice proof:** with the headset connected directly to the modem, place and receive calls, verify caller ID and DTMF, and confirm that VoLTE remains registered after a reboot. LTE data/SMS registration alone does not prove voice service.
2. **V1 — local digital audio:** prove two-way modem audio on the Pi without acoustic coupling or unexplained level/noise problems.
3. **V2 — local WebRTC:** complete two-way audio on the same LAN and measure latency, jitter, packet loss, and one-way-audio failures.
4. **V3 — remote outgoing call:** iPhone on cellular data → TURN/WebRTC → Pi → China Mobile call. Verify Chinese-number caller ID, audio, DTMF, and hang-up.
5. **V4 — remote incoming call:** modem ring → relay → short-lived VoIP push → CallKit screen → media connection → modem answer. This must finish before the cellular caller times out or gives up.
6. **V5 — hardening:** test a locked/terminated iPhone app, Wi-Fi↔cellular changes, caller cancellation, missed/rejected/busy calls, relay failure, power failure mid-call, and extended calls.

#### Voice-specific safeguards

- Block emergency-number dialing: the cellular call originates at the China gateway even when the user is physically in the US, creating dangerous location ambiguity.
- Rate-limit dial attempts and maximum call duration to reduce fraud and accidental charges.
- Never promise caller-ID presentation; it remains subject to China Mobile and the destination carrier.
- Do not record calls by default. Any later recording feature requires explicit consent and a separate legal/privacy review.
- Treat cross-border latency and reachability as field-test questions. Selecting a TURN region requires measurements from both the final China home network and the iPhone’s US networks.

Voice becomes eligible for implementation only after the SMS gateway passes its 30-day reliability gate. A rough single-developer expectation is 1–2 weeks for a basic outgoing prototype, another 2–4 weeks for reliable incoming CallKit behavior, and several additional weeks for cross-border testing and unattended hardening.

### Recommended priority order

1. Base two-way SMS plus durable queueing.
2. Gateway heartbeat, health card, push alerts, and a recovery-state history.
3. Controlled self-healing and encrypted backup/restore.
4. Safe remote updates and session revocation.
5. Quality-of-life features such as OTP focus mode, search, and scheduled summaries.
6. Phase 2 voice feasibility and implementation, after the SMS reliability gate.

## 10. Decision gate before buying production quantities

Buy one recommended prototype only. Proceed to a refined China deployment enclosure after all of these are true:

- The China Mobile SIM registers and exchanges SMS in the SIM7600G-H.
- The end-to-end iPhone test passes at least 20 times across several days.
- A 60-minute power-loss test and all network/reboot recovery tests pass.
- You have a tested plan for a local helper or the iPhone 13 fall-back during the first China month.

## References

- [Apple: China-mainland eSIM constraints](https://support.apple.com/en-sg/123879)
- [Raspberry Pi 4 product brief](https://datasheets.raspberrypi.com/rpi4/raspberry-pi-4-product-brief.pdf)
- [Waveshare SIM7600G-H product page and band table](https://www.waveshare.com/SIM7600G-H-4G-HAT.htm)
- [Waveshare modem board wiki and Raspberry Pi examples](https://www.waveshare.com/wiki/SIM7600G-H_4G_HAT_%28B%29)
- [SIMCom SIM7600H family overview](https://en.simcom.com/product/SIM7600X-H.html)
- [SIMCom SIM7600-series USB-audio application note](https://files.waveshare.com/upload/8/8e/SIM7100_SIM7500_SIM7600_Series_USB_AUDIO_Application_Note_V1.03.pdf)
- [ModemManager documentation](https://modemmanager.org/docs/modemmanager/)
- [Apple CallKit documentation](https://developer.apple.com/documentation/CallKit)
- [Apple PushKit VoIP notification guidance](https://developer.apple.com/documentation/pushkit/responding-to-voip-notifications-from-pushkit)
- [WebRTC TURN server guidance](https://webrtc.org/getting-started/turn-server)
