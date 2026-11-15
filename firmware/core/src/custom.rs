use crate::{
    keyword::{keyword, parse_keyword},
    settings::InvalidValue,
    waveform::MIDDLE,
};
use core::{
    fmt::{self, Write},
    sync::atomic::{AtomicI16, AtomicU16, Ordering::Relaxed},
};

pub const SLOTS: usize = 16;
pub const MAX_SAMPLES: usize = 1024;
pub const RECORD_BYTES: usize = HEADER_BYTES + 2 * MAX_SAMPLES + CRC_BYTES;

const MAGIC: [u8; 4] = *b"PDW1";
const HEADER_BYTES: usize = MAGIC.len() + 2;
const CRC_BYTES: usize = 4;
const LENGTH_MASK: u16 = 0x07FF;
const I8_FLAG: u16 = 1 << 15;
const SIGN_BIT: u16 = 1 << 15;
const CODE_SHIFT: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    I8,
    I16,
}

const FORMATS: [(&str, Format); 2] = [("I8", Format::I8), ("I16", Format::I16)];

impl Format {
    pub fn parse(text: &[u8]) -> Option<Self> {
        parse_keyword(text, &FORMATS)
    }

    const fn digits(self) -> usize {
        match self {
            Format::I8 => 2,
            Format::I16 => 4,
        }
    }

    const fn of(header: u16) -> Self {
        if header & I8_FLAG != 0 {
            Format::I8
        } else {
            Format::I16
        }
    }

    const fn header(self, length: usize) -> u16 {
        let flag = match self {
            Format::I8 => I8_FLAG,
            Format::I16 => 0,
        };
        flag | length as u16
    }

    fn decode(self, digits: &[u8]) -> i16 {
        let value = digits.iter().fold(0u16, |value, &digit| {
            value << 4 | char::from(digit).to_digit(16).map_or(0, |d| d as u16)
        });
        match self {
            Format::I8 => i16::from(value as u8 as i8) << 8,
            Format::I16 => value as i16,
        }
    }
}

pub fn parse_slot(text: &[u8]) -> Option<u8> {
    if text.is_empty() || text.len() > 2 || !text.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let slot = text
        .iter()
        .fold(0, |value, &digit| value * 10 + usize::from(digit - b'0'));
    (slot < SLOTS).then_some(slot as u8)
}

pub struct Slot {
    header: AtomicU16,
    samples: [AtomicI16; MAX_SAMPLES],
}

impl Slot {
    const fn new() -> Self {
        Self {
            header: AtomicU16::new(0),
            samples: [const { AtomicI16::new(0) }; MAX_SAMPLES],
        }
    }

    #[inline]
    pub fn sample(&self, phase: u32) -> u16 {
        let length = u32::from(self.header.load(Relaxed) & LENGTH_MASK);
        if length == 0 {
            return MIDDLE;
        }
        let index = ((phase >> 16) * length) >> 16;
        let sample = self.samples[index as usize].load(Relaxed);
        (sample as u16 ^ SIGN_BIT) >> CODE_SHIFT
    }

    pub fn set(&self, format: Format, hex: &[u8]) -> Result<(), InvalidValue> {
        let width = format.digits();
        let length = hex.len() / width;
        if !hex.len().is_multiple_of(width)
            || !(1..=MAX_SAMPLES).contains(&length)
            || !hex.iter().all(u8::is_ascii_hexdigit)
        {
            return Err(InvalidValue);
        }
        let samples = hex.chunks_exact(width).map(|digits| format.decode(digits));
        self.store(format.header(length), samples);
        Ok(())
    }

    pub fn clear(&self) {
        self.header.store(0, Relaxed);
    }

    fn store(&self, header: u16, samples: impl Iterator<Item = i16>) {
        for (slot, sample) in self.samples.iter().zip(samples) {
            slot.store(sample, Relaxed);
        }
        self.header.store(header, Relaxed);
    }

    fn encode<'r>(&self, record: &'r mut [u8; RECORD_BYTES]) -> &'r [u8] {
        let header = self.header.load(Relaxed);
        let length = usize::from(header & LENGTH_MASK);
        if length == 0 {
            return &[];
        }
        let end = HEADER_BYTES + 2 * length;
        record[..MAGIC.len()].copy_from_slice(&MAGIC);
        record[MAGIC.len()..HEADER_BYTES].copy_from_slice(&header.to_le_bytes());
        let (chunks, _) = record[HEADER_BYTES..end].as_chunks_mut::<2>();
        for (bytes, sample) in chunks.iter_mut().zip(&self.samples) {
            bytes.copy_from_slice(&sample.load(Relaxed).to_le_bytes());
        }
        let crc = crc32(&record[..end]);
        record[end..end + CRC_BYTES].copy_from_slice(&crc.to_le_bytes());
        &record[..end + CRC_BYTES]
    }

    fn restore(&self, record: &[u8]) {
        match parse_record(record) {
            Some((header, samples)) => {
                let (chunks, _) = samples.as_chunks::<2>();
                self.store(
                    header,
                    chunks.iter().map(|&bytes| i16::from_le_bytes(bytes)),
                );
            }
            None => self.clear(),
        }
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = self.header.load(Relaxed);
        let length = usize::from(header & LENGTH_MASK);
        if length == 0 {
            return f.write_str("EMPTY");
        }
        let format = Format::of(header);
        f.write_str(keyword(format, &FORMATS))?;
        f.write_char(' ')?;
        self.samples[..length].iter().try_for_each(|sample| {
            let sample = sample.load(Relaxed);
            match format {
                Format::I8 => write!(f, "{:02X}", (sample >> 8) as u8),
                Format::I16 => write!(f, "{:04X}", sample as u16),
            }
        })
    }
}

fn parse_record(record: &[u8]) -> Option<(u16, &[u8])> {
    let (magic, rest) = record.split_first_chunk::<4>()?;
    let (header, rest) = rest.split_first_chunk::<2>()?;
    let header = u16::from_le_bytes(*header);
    let length = usize::from(header & LENGTH_MASK);
    if *magic != MAGIC
        || header & !(LENGTH_MASK | I8_FLAG) != 0
        || !(1..=MAX_SAMPLES).contains(&length)
    {
        return None;
    }
    let (samples, rest) = rest.split_at_checked(2 * length)?;
    let (crc, _) = rest.split_first_chunk::<CRC_BYTES>()?;
    let end = HEADER_BYTES + 2 * length;
    (u32::from_le_bytes(*crc) == crc32(&record[..end])).then_some((header, samples))
}

fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg())
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashError;

pub trait SlotFlash {
    fn read(&mut self, slot: usize, record: &mut [u8; RECORD_BYTES]) -> Result<(), FlashError>;
    fn erase(&mut self, slot: usize) -> Result<(), FlashError>;
    fn write(&mut self, slot: usize, record: &[u8]) -> Result<(), FlashError>;
}

pub struct Bank([Slot; SLOTS]);

impl Bank {
    pub const fn new() -> Self {
        Self([const { Slot::new() }; SLOTS])
    }

    pub fn slot(&self, index: u8) -> &Slot {
        &self.0[usize::from(index)]
    }

    pub fn clear(&self) {
        self.0.iter().for_each(Slot::clear);
    }

    pub fn load(&self, flash: &mut impl SlotFlash) -> Result<(), FlashError> {
        let mut record = [0; RECORD_BYTES];
        for (index, slot) in self.0.iter().enumerate() {
            flash.read(index, &mut record)?;
            slot.restore(&record);
        }
        Ok(())
    }

    pub fn stale_slots(&self, flash: &mut impl SlotFlash) -> Result<u16, FlashError> {
        let mut encoded = [0; RECORD_BYTES];
        let mut stored = [0; RECORD_BYTES];
        let mut stale = 0;
        for (index, slot) in self.0.iter().enumerate() {
            let wanted = slot.encode(&mut encoded);
            flash.read(index, &mut stored)?;
            let current = if wanted.is_empty() {
                parse_record(&stored).is_none()
            } else {
                stored.starts_with(wanted)
            };
            if !current {
                stale |= 1 << index;
            }
        }
        Ok(stale)
    }

    pub fn rewrite(&self, slots: u16, flash: &mut impl SlotFlash) -> Result<(), FlashError> {
        let mut encoded = [0; RECORD_BYTES];
        for (index, slot) in self.0.iter().enumerate() {
            if slots & 1 << index == 0 {
                continue;
            }
            flash.erase(index)?;
            let record = slot.encode(&mut encoded);
            if !record.is_empty() {
                flash.write(index, record)?;
            }
        }
        Ok(())
    }
}

impl Default for Bank {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
pub(crate) struct MemoryFlash {
    pub sectors: Vec<Vec<u8>>,
    pub erased: Vec<usize>,
    pub written: Vec<usize>,
}

#[cfg(test)]
impl MemoryFlash {
    pub fn new() -> Self {
        Self {
            sectors: vec![vec![0xFF; RECORD_BYTES]; SLOTS],
            erased: Vec::new(),
            written: Vec::new(),
        }
    }
}

#[cfg(test)]
impl SlotFlash for MemoryFlash {
    fn read(&mut self, slot: usize, record: &mut [u8; RECORD_BYTES]) -> Result<(), FlashError> {
        record.copy_from_slice(&self.sectors[slot]);
        Ok(())
    }

    fn erase(&mut self, slot: usize) -> Result<(), FlashError> {
        self.sectors[slot].fill(0xFF);
        self.erased.push(slot);
        Ok(())
    }

    fn write(&mut self, slot: usize, record: &[u8]) -> Result<(), FlashError> {
        let sector = &mut self.sectors[slot][..record.len()];
        assert!(
            sector.iter().all(|&byte| byte == 0xFF),
            "slot {slot} not erased"
        );
        sector.copy_from_slice(record);
        self.written.push(slot);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Bank, Format, MAX_SAMPLES, MemoryFlash, crc32, parse_slot};
    use crate::{settings::InvalidValue, waveform::MIDDLE};
    use rstest::rstest;

    #[test]
    fn computes_crc32() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[rstest]
    #[case("0", Some(0))]
    #[case("15", Some(15))]
    #[case("07", Some(7))]
    #[case("16", None)]
    #[case("", None)]
    #[case("007", None)]
    #[case("-1", None)]
    #[case("a", None)]
    fn parses_slots(#[case] text: &str, #[case] slot: Option<u8>) {
        assert_eq!(parse_slot(text.as_bytes()), slot);
    }

    #[rstest]
    #[case(Format::I16, "80000000FFFF7FFF", [0, 2048, 2047, 4095])]
    #[case(Format::I8, "8000FF7F", [0, 2048, 2032, 4080])]
    #[case(Format::I16, "8000000fFFF07fff", [0, 2048, 2047, 4095])]
    fn samples_set_values(#[case] format: Format, #[case] hex: &str, #[case] codes: [u16; 4]) {
        let bank = Bank::new();
        let slot = bank.slot(5);
        slot.set(format, hex.as_bytes()).unwrap();
        let phases = [0, 1 << 30, 2 << 30, u32::MAX];
        assert_eq!(phases.map(|phase| slot.sample(phase)), codes);
    }

    #[test]
    fn spreads_any_length_over_the_period() {
        let bank = Bank::new();
        let slot = bank.slot(0);
        slot.set(Format::I16, b"800000007FFF").unwrap();
        assert_eq!(slot.sample(0), 0);
        assert_eq!(slot.sample(0x5555_FFFF), 0);
        assert_eq!(slot.sample(0x5556_0000), 2048);
        assert_eq!(slot.sample(u32::MAX), 4095);
    }

    #[test]
    fn outputs_the_middle_when_empty() {
        let bank = Bank::new();
        bank.slot(1).set(Format::I8, b"7F").unwrap();
        bank.slot(1).clear();
        assert_eq!(bank.slot(1).sample(0), MIDDLE);
        assert_eq!(bank.slot(1).to_string(), "EMPTY");
    }

    #[rstest]
    #[case(Format::I8, "80FF007F", "I8 80FF007F")]
    #[case(Format::I16, "abcd0123", "I16 ABCD0123")]
    fn formats_contents(#[case] format: Format, #[case] hex: &str, #[case] shown: &str) {
        let bank = Bank::new();
        bank.slot(2).set(format, hex.as_bytes()).unwrap();
        assert_eq!(bank.slot(2).to_string(), shown);
    }

    #[test]
    fn accepts_the_longest_waveform() {
        let bank = Bank::new();
        let hex = "7FFF".repeat(MAX_SAMPLES);
        bank.slot(15).set(Format::I16, hex.as_bytes()).unwrap();
        assert_eq!(bank.slot(15).to_string(), format!("I16 {hex}"));
    }

    #[rstest]
    #[case(Format::I8, "")]
    #[case(Format::I8, "123")]
    #[case(Format::I16, "12345")]
    #[case(Format::I8, "GG")]
    #[case(Format::I16, "-001")]
    fn rejects_bad_data_without_changing(#[case] format: Format, #[case] hex: &str) {
        let bank = Bank::new();
        bank.slot(3).set(Format::I8, b"01").unwrap();
        assert_eq!(bank.slot(3).set(format, hex.as_bytes()), Err(InvalidValue));
        assert_eq!(bank.slot(3).to_string(), "I8 01");
    }

    #[test]
    fn rejects_too_many_samples() {
        let bank = Bank::new();
        let hex = "00".repeat(MAX_SAMPLES + 1);
        assert_eq!(
            bank.slot(0).set(Format::I8, hex.as_bytes()),
            Err(InvalidValue)
        );
    }

    #[test]
    fn saves_only_changed_slots() {
        let bank = Bank::new();
        let mut flash = MemoryFlash::new();
        assert_eq!(bank.stale_slots(&mut flash), Ok(0));

        bank.slot(3).set(Format::I8, b"80007F").unwrap();
        bank.slot(9).set(Format::I16, b"1234").unwrap();
        let stale = bank.stale_slots(&mut flash).unwrap();
        assert_eq!(stale, 1 << 3 | 1 << 9);
        bank.rewrite(stale, &mut flash).unwrap();
        assert_eq!(flash.erased, [3, 9]);
        assert_eq!(flash.written, [3, 9]);
        assert_eq!(bank.stale_slots(&mut flash), Ok(0));

        bank.slot(9).clear();
        assert_eq!(bank.stale_slots(&mut flash), Ok(1 << 9));
        bank.rewrite(1 << 9, &mut flash).unwrap();
        assert_eq!(flash.erased, [3, 9, 9]);
        assert_eq!(flash.written, [3, 9]);
    }

    #[test]
    fn loads_saved_slots() {
        let saved = Bank::new();
        let mut flash = MemoryFlash::new();
        saved.slot(0).set(Format::I16, b"8000FFFF").unwrap();
        saved.slot(15).set(Format::I8, b"7F80").unwrap();
        saved.rewrite(u16::MAX, &mut flash).unwrap();

        let loaded = Bank::new();
        loaded.slot(4).set(Format::I8, b"00").unwrap();
        loaded.load(&mut flash).unwrap();
        assert_eq!(loaded.slot(0).to_string(), "I16 8000FFFF");
        assert_eq!(loaded.slot(15).to_string(), "I8 7F80");
        assert_eq!(loaded.slot(4).to_string(), "EMPTY");
    }

    #[test]
    fn loads_a_corrupt_record_as_empty() {
        let saved = Bank::new();
        let mut flash = MemoryFlash::new();
        saved.slot(7).set(Format::I16, b"12345678").unwrap();
        saved.rewrite(1 << 7, &mut flash).unwrap();
        flash.sectors[7][8] ^= 1;

        let loaded = Bank::new();
        loaded.load(&mut flash).unwrap();
        assert_eq!(loaded.slot(7).to_string(), "EMPTY");
        assert_eq!(saved.stale_slots(&mut flash), Ok(1 << 7));
    }
}
