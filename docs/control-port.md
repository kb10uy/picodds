# Control Port Protocol

picodds appears on the PC as a USB CDC ACM serial port (VID 0x1209, PID
0x0010, a pid.codes test PID). The text protocol below is implemented in
`picodds_core::control`. The baud rate and other line settings are ignored.

## Framing

- Input is assembled into lines. CR, LF and CRLF each end a line: an LF
  right after a CR is dropped. Empty lines are ignored.
- BS (0x08) and DEL (0x7F) delete the last character of the line being
  assembled.
- Command names, setting names and keyword values are not case-sensitive.
- The firmware does not echo its input. Turn on the terminal's local echo.
- Each command is answered with one line ending in CR LF: `OK`, `OK`
  followed by values, or `ERR` followed by a reason a person can read.
- A line holds up to 64 bytes, not counting its end. A longer line is not
  run and is answered with `ERR line too long`.

## Commands

A command is its name followed by arguments separated by one or more spaces.
Spaces at the end of the line are ignored.

| Command | Action |
|---|---|
| `SET <name> <value>` | Change a setting. The output follows at once |
| `GET <name>` | Reply `OK <value>` |
| `GET` | Reply every setting on one line, `OK WAVE=SINE FREQ=1000 AMP=1` |
| `DEFAULTS` | Set every setting to its default |
| `VER` | Reply the firmware version |
| `BOOTSEL` | Reply `OK` and reboot into BOOTSEL mode for flashing |
| `HELP` | List the command names on one line |

## Settings

| Name | Values | Default | Meaning |
|---|---|---|---|
| `WAVE` | `SINE`, `SQUARE`, `TRIANGLE`, `SAW` | `SINE` | Waveform |
| `FREQ` | Hz, `0.001` to `100000`, up to three decimals | `1000` | Frequency |
| `AMP` | Vp-p, `0` to `2`, up to three decimals | `1` | Amplitude at the output pins, unloaded |

- `GET` shows keywords in upper case and numbers without trailing zeros.
- Numbers are plain decimals: no sign, exponent or unit suffix.
- Settings are not stored. Power-up starts from the defaults.
- Changing `FREQ` or `WAVE` keeps the phase continuous.
- `AMP` assumes the 3.3 V rail of the XIAO RP2040 is exactly 3.3 V, so the
  real amplitude is off by the rail's error.

## Errors

`ERR` is followed by one of `unknown command`, `bad argument`,
`unknown setting`, `bad value` or `line too long`. A command answered with
`ERR` changes nothing.

## Example

```text
> SET WAVE SQUARE
OK
> SET FREQ 440
OK
> SET AMP 0.5
OK
> GET
OK WAVE=SQUARE FREQ=440 AMP=0.5
```
