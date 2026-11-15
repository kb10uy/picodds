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
- A line holds up to 4160 bytes, not counting its end, which fits a whole
  custom waveform. A longer line is not run and is answered with
  `ERR line too long`.

## Commands

A command is its name followed by arguments separated by one or more spaces.
Spaces at the end of the line are ignored.

| Command | Action |
|---|---|
| `SET <name> <value>` | Change a setting. The output follows at once |
| `GET <name>` | Reply `OK <value>` |
| `GET` | Reply every setting on one line, `OK WAVE=SINE FREQ=1000 AMP=1` |
| `DEFAULTS` | Set every setting to its default. Custom waveforms are kept |
| `WSET <slot> <format> <hex>` | Replace the waveform in a slot |
| `WGET <slot>` | Reply `OK <format> <hex>`, or `OK EMPTY` |
| `WCLEAR <slot>` | Empty a slot in memory. Flash is not touched |
| `WSAVE` | Save every slot to flash |
| `WWIPE` | Empty every slot, in memory and in flash |
| `VER` | Reply the firmware version |
| `BOOTSEL` | Reply `OK`, hold the output at the middle and reboot into BOOTSEL mode for flashing |
| `HELP` | List the command names on one line |

## Settings

| Name | Values | Default | Meaning |
|---|---|---|---|
| `WAVE` | `SINE`, `SQUARE`, `TRIANGLE`, `SAW`, `W0` to `W15` | `SINE` | Waveform. `W<n>` plays custom slot n |
| `FREQ` | Hz, `0.001` to `100000`, up to three decimals | `1000` | Frequency |
| `AMP` | Vp-p, `0` to `2`, up to three decimals | `1` | Amplitude at the output pins, unloaded |

- `GET` shows keywords in upper case and numbers without trailing zeros.
- Numbers are plain decimals: no sign, exponent or unit suffix.
- Settings are not stored. Power-up starts from the defaults.
- Changing `FREQ` or `WAVE` keeps the phase continuous.
- `AMP` assumes the 3.3 V rail of the XIAO RP2040 is exactly 3.3 V, so the
  real amplitude is off by the rail's error.

## Custom Waveforms

There are 16 slots, numbered `0` to `15`. A slot holds 1 to 1024 samples
spread evenly over one period, or nothing.

- `<format>` is `I8` or `I16`. The data is the samples in order, each in
  two's complement hex with no separators: two digits for `I8`, four for
  `I16`. Hex digits are not case-sensitive; `WGET` replies in upper case.
- A sample of 0 is the middle of the output. `I8` samples are shifted up by
  4 bits and `I16` samples down by 4 bits to make the 12-bit DAC code, so
  `I8` `7F` gives 4080 and `I16` `7FFF` gives 4095.
- `WGET` replies in the format the slot was set with, with the same digits.
- An empty slot outputs the middle. A slot that is playing changes at once
  when it is set or cleared.
- `WSET` with a bad slot, format or data changes nothing.

Slots are saved to flash only by `WSAVE` and `WWIPE`, and are loaded from
flash at power-up. A saved slot that cannot be read back starts empty.

`WSAVE` and `WWIPE` rewrite only the slots whose flash copy differs. While
they do, the output is held at the middle, and the control port does not
answer, for tens of milliseconds or more per rewritten slot. Nothing is
held when no slot differs.

## Errors

`ERR` is followed by one of `unknown command`, `bad argument`,
`unknown setting`, `bad value`, `line too long` or `flash error`. A command
answered with `ERR` changes nothing, except that `WSAVE` or `WWIPE` failing
with `flash error` may have rewritten some slots.

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
> WSET 3 I8 007FFF80
OK
> SET WAVE W3
OK
> WGET 3
OK I8 007FFF80
> WSAVE
OK
```
