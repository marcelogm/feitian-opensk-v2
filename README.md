# OpenSK on the Feitian OpenSK USB Dongle V2

Turns a **Feitian OpenSK USB Dongle V2** (metal case, one button) into a working FIDO2 key with
[OpenSK](https://github.com/google/OpenSK), which runs on [Wasefire](https://github.com/google/wasefire).
Everything builds and flashes inside Docker.

On some units the button is wired to the chip's reset pin: every touch reboots the dongle into
the bootloader, and OpenSK never sees it. This repository fixes that.

> [!WARNING]
> The fix is **irreversible without an SWD probe**. Afterwards the dongle can never enter DFU mode
> again. It can only be updated through Wasefire's USB update, and it can only run Wasefire/OpenSK.

## Do you need this?

1. Put the dongle in DFU mode: the red LED breathes, and `lsusb` shows `1915:521f`.
2. Tap the button once.

If it disconnects and comes back as `1915:521f`, you need the fix. If nothing happens, stock
OpenSK works: do steps 1–3 only.

## Requirements

Linux, Docker, `git`, `lsusb`. Containers run `--privileged` with `/dev` mounted for USB access.

## Procedure

```sh
scripts/00-setup.sh               # build the image, clone OpenSK (pinned), apply the patch
scripts/10-build-dfu-packages.sh  # work/out/dongle-1.zip and dongle-2.zip (stock OpenSK)
```

**Install stock OpenSK.** Tap the button before each flash to enter DFU:

```sh
scripts/20-dfu-flash.sh work/out/dongle-1.zip
scripts/20-dfu-flash.sh work/out/dongle-2.zip
```

The dongle now shows as `18d1:0239 Google Inc. Wasefire`. Touches still reboot it.

**Apply the fix (irreversible).** Each update writes the other A/B slot, so run it twice:

```sh
scripts/40-platform-update.sh   # type "yes"; then unplug and replug the dongle
scripts/30-touch-test.sh        # tap when the LED blinks green -> "OK: touch recognized"
scripts/40-platform-update.sh   # second slot
scripts/30-touch-test.sh
```

Then set a PIN (Chrome: `chrome://settings/securityKeys`) and test at <https://webauthn.io>.

## Afterwards

- **Updating OpenSK.** Use only `scripts/40-platform-update.sh`.
  1. Update one slot, replug and test.
  2. Then update the other slot.
  If a slot misbehaves, replugging 3 times boots the other one.
- **DFU only comes back with SWD.** The SWDIO/SWDCLK pads are inside the case.
- **Credentials and PIN** can still be reset from Chrome.
- **This is not a certified authenticator.** OpenSK is a research platform, so keep a backup key.

## How it works

See [docs/findings.md](docs/findings.md). In short:

- The bootloader enters DFU on any pin reset.
- At every boot it re-enables the reset pin if it finds it disabled.
- It only accepts signed bootloader updates.

The nRF52840, however, disables the reset pin when the two `UICR.PSELRESET` registers hold
different values. A patched Wasefire runner writes `PSELRESET[1] = 0x10` (`PSELRESET[0]` stays
`0x12`) on its first boot, and reads the button on P0.18. That runner is installed through
Wasefire's USB update, which does not need DFU.

## Layout

| Path | What |
|---|---|
| `scripts/` | the procedure; `common.sh` pins OpenSK `636d85b`, Wasefire `6d8f4fc` |
| `patches/wasefire-feitian-unpin.patch` | runner patch: features `feitian-unpin`, `feitian-button-p018` |
| `uicr-fix/` | optional diagnostic firmware: UICR report and bootloader dump over USB serial (`scripts/50-uicr-report.sh`, before the fix only). Don't use its `FIX`/`UNPIN` commands. |
| `tools/touch_test.py` | CTAP2 touch test |

## License

Apache-2.0, see [LICENSE](LICENSE). The patch modifies [Wasefire](https://github.com/google/wasefire), also Apache-2.0.
