# Milestone one: hardware proof and proxy foundation

Goal: prove receiving SMS, sending SMS, receiving calls, and making calls on the actual Pi/SIM7600 hardware, and establish a testable device-to-cloud connection. The hardware proof remains incomplete until all four live operations and two-way audio pass.

## Sub-steps

| Step | Work | Completion evidence |
|---|---|---|
| 1 | Baseline USB, SIM, signal, registration and audio capability diagnostics | Saved sanitized JSON and repeatable script |
| 2 | Select cloud account, budget and region; deploy diagnostics proxy | Authenticated Pi upload, server persistence, authenticated read |
| 3 | Receive a controlled test SMS | Known participant sends marker text; verify sender/content once, without deleting other messages |
| 4 | Send a controlled test SMS | Approved recipient receives marker; record submitted vs delivered separately |
| 5 | Place and receive cellular calls | Approved participants confirm ringing, answer, hang-up and DTMF |
| 6 | Prove modem-to-Pi full-duplex audio | 10-minute two-way test; record sample format, path and audio failures |
| 7 | Repeat after restart and publish evidence | Three SMS round trips and three calls each direction; documented results |

## Implemented now

- `device/probe.py`: bounded AT queries with serial restoration, stable port discovery, conservative parsing, no cellular side effects.
- `device/publish_report.py`: authenticated HTTPS upload (localhost HTTP allowed for an SSH tunnel); redirects refused.
- `cloud/app.py`: device-only report writes, admin-only reads, persistent latest report, replay and stale-report protection, payload limits.
- Container definition, loopback-only Compose deployment and tests for parser behavior, authorization, validation and persistence.

## Hardware observations and open audio question

Earlier session confirmed SIM READY and LTE roaming registration. Current inspection exposes SimTech interfaces 00–04 and only HDMI ALSA cards. A missing ALSA sound card does not prove missing USB audio: SIMCom describes a separate USB audio data interface, and a Waveshare-hosted Linux interface table identifies interface 4 as USB audio. Confirm applicability to this exact firmware before enabling it.

The read-only `AT+CPCMREG=?` capability query is included. A supported response is only command support, not proof of working capture/playback. An ERROR or missing structured response is inconclusive. Do not issue USB mode-switch commands based only on another board's example.

References:
- [SIMCom USB audio application note](https://files.waveshare.com/upload/8/8e/SIM7100_SIM7500_SIM7600_Series_USB_AUDIO_Application_Note_V1.03.pdf)
- [Linux USB interface table](https://files.waveshare.com/upload/0/01/SIM7500_SIM7600_Series_Linux_NDIS_driver_V2.00.pdf)
- [Waveshare HAT B guide](https://www.waveshare.com/wiki/SIM7600G-H_4G_HAT_%28B%29)

## Live test procedure and prerequisites

Obtain the owner's designated test number and permission for short calls/messages first. Use a neutral marker, never a banking OTP. Keep test numbers and message contents outside Git. The current code intentionally provides no arbitrary AT endpoint or send/dial command.

For receive testing, agree when the participant will send the marker and call. Implement controlled SMS read and call-event capture after permission; preserve existing modem messages. For send/dial testing, use that same explicitly approved number, with a bounded timeout and guaranteed hang-up cleanup. Do not auto-retry uncertain SMS submissions or redial. The owner confirms audible two-way voice; modem OK responses cannot substitute for that observation.

Cloud account choice/access and live test authorization are separate dependencies. Diagnostics and local proxy tests can proceed without either. No paid cloud resources should be provisioned until a concrete provider/size/region/cost proposal is accepted.

## Validation performed on 2026-10-05 (Pacific time)

- Automated tests passed for fragmented AT responses, serial restoration after timeout, rejected dial commands, HTTPS enforcement, device/admin role separation, report replay/conflicts, stale/future payloads, unknown fields, body limits, and persistence after app recreation.
- The new probe ran on the real Pi: SIM READY, LTE registration status 5 (roaming), CSQ 27, SMS PDU mode 0, USB audio capability query supported, no query errors. This is a point-in-time observation.
- A real Pi probe report was uploaded through a temporary SSH tunnel to the local proxy; a separately authenticated admin read returned the persisted report. Temporary proxy and tunnel were stopped afterward. No proxy was deployed to a cloud account.
- No SMS was read/sent/deleted and no call was placed or answered. Full-duplex audio remains unverified.
- Docker is unavailable on the development Mac, so container build/Compose runtime remain unverified until a suitable host is provided.
