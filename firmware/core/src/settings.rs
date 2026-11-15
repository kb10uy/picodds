use crate::{
    custom::parse_slot,
    keyword::{keyword, parse_keyword},
    waveform::Waveform,
};
use core::fmt;

const WAVEFORMS: [(&str, Waveform); 4] = [
    ("SINE", Waveform::Sine),
    ("SQUARE", Waveform::Square),
    ("TRIANGLE", Waveform::Triangle),
    ("SAW", Waveform::Sawtooth),
];
const MAX_WHOLE_DIGITS: usize = 9;
const MAX_FRACTION_DIGITS: usize = 3;

fn parse_waveform(text: &[u8]) -> Option<Waveform> {
    parse_keyword(text, &WAVEFORMS).or_else(|| match text {
        [b'W' | b'w', slot @ ..] => parse_slot(slot).map(Waveform::Custom),
        _ => None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frequency(u32);

impl Frequency {
    pub const MIN: Frequency = Frequency(1);
    pub const MAX: Frequency = Frequency(100_000_000);

    pub const fn millihertz(self) -> u32 {
        self.0
    }

    fn parse(text: &[u8]) -> Option<Self> {
        parse_milli(text)
            .filter(|mhz| (u64::from(Self::MIN.0)..=u64::from(Self::MAX.0)).contains(mhz))
            .map(|mhz| Frequency(mhz as u32))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amplitude(u16);

impl Amplitude {
    pub const MAX: Amplitude = Amplitude(2_000);

    pub const fn millivolts(self) -> u16 {
        self.0
    }

    fn parse(text: &[u8]) -> Option<Self> {
        parse_milli(text)
            .filter(|&mv| mv <= u64::from(Self::MAX.0))
            .map(|mv| Amplitude(mv as u16))
    }
}

fn parse_milli(text: &[u8]) -> Option<u64> {
    let (whole, fraction) = match text.iter().position(|&b| b == b'.') {
        Some(dot) => (&text[..dot], &text[dot + 1..]),
        None => (text, &b"0"[..]),
    };
    if whole.is_empty()
        || whole.len() > MAX_WHOLE_DIGITS
        || fraction.is_empty()
        || fraction.len() > MAX_FRACTION_DIGITS
        || !whole.iter().chain(fraction).all(u8::is_ascii_digit)
    {
        return None;
    }
    let digits = |digits: &[u8]| {
        digits
            .iter()
            .fold(0, |value, &digit| value * 10 + u64::from(digit - b'0'))
    };
    let scale = 10u64.pow((MAX_FRACTION_DIGITS - fraction.len()) as u32);
    Some(digits(whole) * 1_000 + digits(fraction) * scale)
}

fn write_milli(f: &mut fmt::Formatter<'_>, value: u32) -> fmt::Result {
    let (whole, mut fraction) = (value / 1_000, value % 1_000);
    write!(f, "{whole}")?;
    if fraction == 0 {
        return Ok(());
    }
    let mut digits = MAX_FRACTION_DIGITS;
    while fraction % 10 == 0 {
        fraction /= 10;
        digits -= 1;
    }
    write!(f, ".{fraction:0digits$}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub waveform: Waveform,
    pub frequency: Frequency,
    pub amplitude: Amplitude,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            waveform: Waveform::Sine,
            frequency: Frequency(1_000_000),
            amplitude: Amplitude(1_000),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Wave,
    Freq,
    Amp,
}

impl Setting {
    pub const ALL: [Setting; 3] = [Setting::Wave, Setting::Freq, Setting::Amp];

    pub const fn name(self) -> &'static str {
        match self {
            Setting::Wave => "WAVE",
            Setting::Freq => "FREQ",
            Setting::Amp => "AMP",
        }
    }

    pub fn from_name(name: &[u8]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|setting| name.eq_ignore_ascii_case(setting.name().as_bytes()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidValue;

impl Settings {
    pub fn set(&mut self, setting: Setting, value: &[u8]) -> Result<(), InvalidValue> {
        let applied = match setting {
            Setting::Wave => parse_waveform(value).map(|wave| self.waveform = wave),
            Setting::Freq => Frequency::parse(value).map(|freq| self.frequency = freq),
            Setting::Amp => Amplitude::parse(value).map(|amp| self.amplitude = amp),
        };
        applied.ok_or(InvalidValue)
    }

    pub fn value(&self, setting: Setting) -> Value<'_> {
        Value {
            settings: self,
            setting,
        }
    }
}

pub struct Value<'a> {
    settings: &'a Settings,
    setting: Setting,
}

impl fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.settings;
        match self.setting {
            Setting::Wave => match s.waveform {
                Waveform::Custom(slot) => write!(f, "W{slot}"),
                waveform => f.write_str(keyword(waveform, &WAVEFORMS)),
            },
            Setting::Freq => write_milli(f, s.frequency.0),
            Setting::Amp => write_milli(f, u32::from(s.amplitude.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InvalidValue, Setting, Settings};
    use crate::waveform::Waveform;
    use rstest::rstest;

    #[rstest]
    #[case(Setting::Wave, "sine", "SINE")]
    #[case(Setting::Wave, "Square", "SQUARE")]
    #[case(Setting::Wave, "TRIANGLE", "TRIANGLE")]
    #[case(Setting::Wave, "saw", "SAW")]
    #[case(Setting::Wave, "W0", "W0")]
    #[case(Setting::Wave, "w15", "W15")]
    #[case(Setting::Wave, "W07", "W7")]
    #[case(Setting::Freq, "1000", "1000")]
    #[case(Setting::Freq, "0.001", "0.001")]
    #[case(Setting::Freq, "440.0", "440")]
    #[case(Setting::Freq, "1234.50", "1234.5")]
    #[case(Setting::Freq, "100000", "100000")]
    #[case(Setting::Freq, "007", "7")]
    #[case(Setting::Amp, "0", "0")]
    #[case(Setting::Amp, "0.5", "0.5")]
    #[case(Setting::Amp, "1.25", "1.25")]
    #[case(Setting::Amp, "2", "2")]
    #[case(Setting::Amp, "2.000", "2")]
    fn sets_and_formats(#[case] setting: Setting, #[case] value: &str, #[case] shown: &str) {
        let mut settings = Settings::default();
        settings.set(setting, value.as_bytes()).unwrap();
        assert_eq!(settings.value(setting).to_string(), shown);
    }

    #[rstest]
    #[case(Setting::Wave, "SIN")]
    #[case(Setting::Wave, "")]
    #[case(Setting::Wave, "W")]
    #[case(Setting::Wave, "W16")]
    #[case(Setting::Wave, "W-1")]
    #[case(Setting::Freq, "0")]
    #[case(Setting::Freq, "0.0001")]
    #[case(Setting::Freq, "100000.001")]
    #[case(Setting::Freq, "1e3")]
    #[case(Setting::Freq, "1k")]
    #[case(Setting::Freq, "-1")]
    #[case(Setting::Freq, "+1")]
    #[case(Setting::Freq, ".5")]
    #[case(Setting::Freq, "5.")]
    #[case(Setting::Freq, "1,5")]
    #[case(Setting::Freq, "1.2.3")]
    #[case(Setting::Freq, "99999999999999999999")]
    #[case(Setting::Amp, "2.001")]
    #[case(Setting::Amp, "3")]
    #[case(Setting::Amp, "1V")]
    fn rejects_invalid_values(#[case] setting: Setting, #[case] value: &str) {
        let mut settings = Settings::default();
        assert_eq!(settings.set(setting, value.as_bytes()), Err(InvalidValue));
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn has_defaults() {
        let settings = Settings::default();
        assert_eq!(settings.waveform, Waveform::Sine);
        assert_eq!(settings.frequency.millihertz(), 1_000_000);
        assert_eq!(settings.amplitude.millivolts(), 1_000);
    }

    #[rstest]
    #[case("wave", Some(Setting::Wave))]
    #[case("Freq", Some(Setting::Freq))]
    #[case("AMP", Some(Setting::Amp))]
    #[case("OFFSET", None)]
    fn finds_settings_by_name(#[case] name: &str, #[case] setting: Option<Setting>) {
        assert_eq!(Setting::from_name(name.as_bytes()), setting);
    }
}
