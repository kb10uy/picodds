# Firmware

The firmware is written in Rust on embassy for the Seeed XIAO RP2040.

## Layout

- `firmware/core/` (`picodds-core`) is a `no_std` library without platform
  dependencies, tested on the host: the control protocol, the settings, the
  waveform tables and the DDS arithmetic.
- `firmware/app/` (`picodds-firmware`) is the RP2040 binary.

## Signal Path

- PIO0 SM0 drives the MCP4922 over SPI on its own: SCK on D4 (GP6), ~CS on
  D5 (GP7) and SDI on D3 (GP29). LDAC is tied low, so a channel updates when
  ~CS rises.
- One 16-bit word takes 50 PIO cycles. The divider is chosen for 500 kHz,
  which the default 125 MHz system clock gives exactly (SCK 12.5 MHz).
- Core 1 runs the phase accumulator (32 bits) and pushes one word per
  sample into the joined 8-word TX FIFO, so the PIO paces the sample rate.
  The sine table (4096 points, 12 bits) is placed in RAM.
- Channel B sets the amplitude: it is the reference of channel A, and U3A
  outputs VOUTA - VOUTB / 2 + 1.65 V. Core 1 writes channel B in place of
  one sample whenever `AMP` changes. The phase still advances in that slot.
- Core 0 runs USB and the control port, and hands the settings to core 1
  through atomics.

## Build and Test

In `firmware/`:

- `cargo test` runs the host tests of `core`.
- `cargo clippy --all-targets -- -D warnings` checks `core`.
- `cargo fmt --all --check` checks the formatting of both crates.

In `firmware/app/`:

- `cargo build --release` builds the firmware.
- `cargo clippy -- -D warnings` checks the firmware.
- `cargo run --release` loads the firmware with `picotool`. The board has to
  be in BOOTSEL mode first: send `BOOTSEL` to the control port, or hold the
  B button while plugging it in.
