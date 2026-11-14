use crate::{
    settings::{Amplitude, Frequency, Settings},
    waveform::{FULL_SCALE, Waveform},
};

pub const VREF_MILLIVOLTS: u32 = 3_300;

const DAC_STEPS: u32 = 1 << 12;
const CHANNEL_B: u16 = 1 << 15;
const UNITY_GAIN: u16 = 1 << 13;
const ACTIVE: u16 = 1 << 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuning {
    pub step: u32,
    pub waveform: Waveform,
    pub amplitude: u16,
}

impl Tuning {
    pub fn new(settings: &Settings, sample_rate_hz: u32) -> Self {
        Self {
            step: phase_step(settings.frequency, sample_rate_hz),
            waveform: settings.waveform,
            amplitude: amplitude_code(settings.amplitude),
        }
    }
}

fn phase_step(frequency: Frequency, sample_rate_hz: u32) -> u32 {
    let sample_rate_mhz = u64::from(sample_rate_hz) * 1_000;
    let step =
        ((u64::from(frequency.millihertz()) << u32::BITS) + sample_rate_mhz / 2) / sample_rate_mhz;
    u32::try_from(step).unwrap_or(u32::MAX)
}

fn amplitude_code(amplitude: Amplitude) -> u16 {
    let code =
        (u32::from(amplitude.millivolts()) * DAC_STEPS + VREF_MILLIVOLTS / 2) / VREF_MILLIVOLTS;
    code.min(u32::from(FULL_SCALE)) as u16
}

#[inline]
pub const fn channel_a(code: u16) -> u16 {
    UNITY_GAIN | ACTIVE | (code & FULL_SCALE)
}

#[inline]
pub const fn channel_b(code: u16) -> u16 {
    CHANNEL_B | UNITY_GAIN | ACTIVE | (code & FULL_SCALE)
}

#[cfg(test)]
mod tests {
    use super::{Tuning, channel_a, channel_b};
    use crate::settings::{Setting, Settings};
    use rstest::rstest;

    fn tuning(freq: &str, amp: &str, sample_rate_hz: u32) -> Tuning {
        let mut settings = Settings::default();
        settings.set(Setting::Freq, freq.as_bytes()).unwrap();
        settings.set(Setting::Amp, amp.as_bytes()).unwrap();
        Tuning::new(&settings, sample_rate_hz)
    }

    #[rstest]
    #[case("1000", 500_000, 8_589_935)]
    #[case("0.001", 500_000, 9)]
    #[case("100000", 500_000, 858_993_459)]
    #[case("100000", 200_000, 1 << 31)]
    #[case("1000", 250_000, 17_179_869)]
    fn computes_phase_step(#[case] freq: &str, #[case] sample_rate_hz: u32, #[case] step: u32) {
        assert_eq!(tuning(freq, "1", sample_rate_hz).step, step);
    }

    #[test]
    fn saturates_phase_step_above_the_sample_rate() {
        assert_eq!(tuning("100000", "1", 50_000).step, u32::MAX);
    }

    #[rstest]
    #[case("0", 0)]
    #[case("1", 1241)]
    #[case("2", 2482)]
    #[case("0.001", 1)]
    fn computes_amplitude_code(#[case] amp: &str, #[case] code: u16) {
        assert_eq!(tuning("1000", amp, 500_000).amplitude, code);
    }

    #[rstest]
    #[case(0, 0x3000, 0xB000)]
    #[case(0x0FFF, 0x3FFF, 0xBFFF)]
    #[case(0x0ABC, 0x3ABC, 0xBABC)]
    fn builds_dac_words(#[case] code: u16, #[case] a: u16, #[case] b: u16) {
        assert_eq!(channel_a(code), a);
        assert_eq!(channel_b(code), b);
    }
}
