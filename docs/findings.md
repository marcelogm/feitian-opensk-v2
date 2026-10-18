# Findings

## Symptom

- Stock OpenSK booted, but a single short tap disconnected the dongle and brought it back in DFU
  mode (`1915:521f`), even when only the Nordic bootloader was running.
- After the reset pin was disabled, a tap no longer rebooted the dongle, but OpenSK, which reads
  P1.06, never saw the press.
- With the button read on **P0.18**, it worked.

So on this unit the button drives P0.18, the reset pin. Feitian's V2 schematic
(`Z711_G9_KE09_V1.1`) puts it on P1.06, with an RC circuit that only pulls RESET after about 10 s.

## UICR before the fix

```
NRFFW0 = 0x000E0000   NRFFW1 = 0x000FE000   PSELRESET0 = PSELRESET1 = 0x00000012
NFCPINS = 0xFFFFFFFE  REGOUT0 = 0xFFFFFFFC  APPROTECT = 0xFFFFFFFF
```

## Bootloader

The bootloader is Feitian's build of the nRF5 SDK 17.0.2 Open DFU bootloader, at `0xE0000..0xF03A0`.
It was dumped and disassembled with `uicr-fix`.

- **Re-enables pin reset.** `SystemInit` (at `0xE8882`) writes `PSELRESET = 18` to both registers
  and resets if either one is disconnected (bit 31 set). It never checks the pin number.
- **DFU entry.** It enters DFU only on a pin reset or when the app is marked invalid. It does not
  check the button or `GPREGRET`.
- **Signed bootloader updates.** It refuses unsigned bootloader packages, so it cannot be patched
  without Feitian's key.
- **App write protection.** Before starting the app it write-protects that app's region and its own
  region (ACL). The UICR is not protected.

## Fix

nRF52840 Product Specification v1.1, §4.5.1.4:

> All PSELRESET registers have to contain the same value for a pin mapping to be valid. If values
> are not the same, there will be no nRESET function exposed on a GPIO.

Writing `PSELRESET[1] = 0x10` over `0x12` only clears bits, so no UICR erase is needed. Each UICR
word can be written twice between erases, and this is the second write. Both registers stay
"connected", so the bootloader leaves them alone.

The write cannot be done from a standalone app. Without a reset pin there would be no DFU left to
install OpenSK afterwards. So the steps are:

1. Install stock OpenSK over DFU (two packages) while the reset pin still works.
2. Install a patched Wasefire runner through Wasefire's own USB update. The second DFU package
   leaves the platform slots writable.
3. On boot, the runner writes `PSELRESET[1] = 0x10` if it finds `NRFFW0 = 0xE0000` and
   `PSELRESET = 0x12/0x12`, and reads the button on P0.18.
4. Install the patched runner in both A/B slots, since Wasefire falls back to the other slot
   after 3 boots.

## Flashing notes

- **`nrfdfu` fails.** OpenSK's `flash.sh` uses `nrfdfu`, which fails when a SoftDevice is present
  (it sends no `sd_req`) and times out after 1 s on large erases. `nrfutil` with `--sd-req=0` works.

## What didn't work

| Attempt | Why not |
|---|---|
| Erase the UICR and rewrite it without `PSELRESET` | the bootloader writes 18 back at the next boot |
| Patch the bootloader over DFU | bootloader updates must be signed by Feitian |
| Disable the reset pin from a diagnostic app | there would be no DFU left to install OpenSK |
