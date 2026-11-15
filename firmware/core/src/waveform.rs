use crate::custom::{Bank, SLOTS};
use core::f64::consts::FRAC_PI_2;

pub const FULL_SCALE: u16 = 4095;
pub const MIDDLE: u16 = 1 << 11;

const SINE_BITS: u32 = 12;
const SINE_LEN: usize = 1 << SINE_BITS;
const TAYLOR_TERMS: u32 = 10;
const CUSTOM_BASE: u8 = 16;
const CUSTOM_END: u8 = CUSTOM_BASE + SLOTS as u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Square,
    Triangle,
    Sawtooth,
    Custom(u8),
}

impl Waveform {
    #[inline]
    pub const fn to_u8(self) -> u8 {
        match self {
            Waveform::Sine => 0,
            Waveform::Square => 1,
            Waveform::Triangle => 2,
            Waveform::Sawtooth => 3,
            Waveform::Custom(slot) => CUSTOM_BASE + slot,
        }
    }

    #[inline]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Waveform::Square,
            2 => Waveform::Triangle,
            3 => Waveform::Sawtooth,
            CUSTOM_BASE..CUSTOM_END => Waveform::Custom(value - CUSTOM_BASE),
            _ => Waveform::Sine,
        }
    }

    #[inline]
    pub fn sample(self, phase: u32, sine: &SineTable, bank: &Bank) -> u16 {
        match self {
            Waveform::Sine => sine.0[(phase >> (u32::BITS - SINE_BITS)) as usize],
            Waveform::Square if phase < 1 << 31 => FULL_SCALE,
            Waveform::Square => 0,
            Waveform::Triangle => {
                let position = (phase >> 19) as u16;
                if position <= FULL_SCALE {
                    position
                } else {
                    2 * FULL_SCALE + 1 - position
                }
            }
            Waveform::Sawtooth => (phase >> 20) as u16,
            Waveform::Custom(slot) => bank.slot(slot).sample(phase),
        }
    }
}

pub struct SineTable([u16; SINE_LEN]);

impl SineTable {
    pub const fn new() -> Self {
        let mut table = [0; SINE_LEN];
        let mut index = 0;
        while index < SINE_LEN {
            table[index] = sine_code(index);
            index += 1;
        }
        Self(table)
    }
}

impl Default for SineTable {
    fn default() -> Self {
        Self::new()
    }
}

const fn sine_code(index: usize) -> u16 {
    const HALF: usize = SINE_LEN / 2;
    const QUARTER: usize = SINE_LEN / 4;
    let in_half = index % HALF;
    let from_zero = if in_half > QUARTER {
        HALF - in_half
    } else {
        in_half
    };
    let magnitude = sin(from_zero as f64 * (FRAC_PI_2 / QUARTER as f64));
    let value = if index < HALF { magnitude } else { -magnitude };
    let middle = FULL_SCALE as f64 / 2.0;
    (middle + middle * value + 0.5) as u16
}

const fn sin(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    let mut n = 1;
    while n < TAYLOR_TERMS {
        term = -term * x2 / ((2 * n) * (2 * n + 1)) as f64;
        sum += term;
        n += 1;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::{FULL_SCALE, SINE_LEN, SineTable, Waveform};
    use crate::custom::{Bank, Format};
    use core::f64::consts::TAU;
    use rstest::rstest;

    #[test]
    fn sine_table_matches_std() {
        let table = SineTable::new();
        let middle = f64::from(FULL_SCALE) / 2.0;
        for (index, &code) in table.0.iter().enumerate() {
            let angle = TAU * index as f64 / SINE_LEN as f64;
            let expected = (middle + middle * angle.sin() + 0.5).floor() as u16;
            assert_eq!(code, expected, "index {index}");
        }
    }

    #[rstest]
    #[case(Waveform::Sine, 0, 2048)]
    #[case(Waveform::Sine, 1 << 30, FULL_SCALE)]
    #[case(Waveform::Sine, 3 << 30, 0)]
    #[case(Waveform::Square, 0, FULL_SCALE)]
    #[case(Waveform::Square, (1 << 31) - 1, FULL_SCALE)]
    #[case(Waveform::Square, 1 << 31, 0)]
    #[case(Waveform::Square, u32::MAX, 0)]
    #[case(Waveform::Triangle, 0, 0)]
    #[case(Waveform::Triangle, (1 << 31) - 1, FULL_SCALE)]
    #[case(Waveform::Triangle, 1 << 31, FULL_SCALE)]
    #[case(Waveform::Triangle, 1 << 30, 2048)]
    #[case(Waveform::Triangle, 3 << 30, 2047)]
    #[case(Waveform::Triangle, u32::MAX, 0)]
    #[case(Waveform::Sawtooth, 0, 0)]
    #[case(Waveform::Sawtooth, 1 << 31, 2048)]
    #[case(Waveform::Sawtooth, u32::MAX, FULL_SCALE)]
    fn samples_waveforms(#[case] waveform: Waveform, #[case] phase: u32, #[case] code: u16) {
        assert_eq!(
            waveform.sample(phase, &SineTable::new(), &Bank::new()),
            code
        );
    }

    #[test]
    fn samples_custom_slots() {
        let bank = Bank::new();
        bank.slot(15).set(Format::I16, b"7FFF").unwrap();
        let sine = SineTable::new();
        assert_eq!(Waveform::Custom(15).sample(0, &sine, &bank), FULL_SCALE);
        assert_eq!(Waveform::Custom(14).sample(0, &sine, &bank), 2048);
    }

    #[rstest]
    #[case(Waveform::Sine)]
    #[case(Waveform::Square)]
    #[case(Waveform::Triangle)]
    #[case(Waveform::Sawtooth)]
    #[case(Waveform::Custom(0))]
    #[case(Waveform::Custom(15))]
    fn round_trips_through_u8(#[case] waveform: Waveform) {
        assert_eq!(Waveform::from_u8(waveform.to_u8()), waveform);
    }
}
