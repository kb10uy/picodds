use crate::dac::Dac;
use core::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, Ordering};
use picodds_core::{
    control::Generator,
    dds::{self, Tuning},
    settings::Settings,
    waveform::{SineTable, Waveform},
};

pub struct Params {
    step: AtomicU32,
    waveform: AtomicU8,
    amplitude: AtomicU16,
}

impl Params {
    pub const fn new() -> Self {
        Self {
            step: AtomicU32::new(0),
            waveform: AtomicU8::new(0),
            amplitude: AtomicU16::new(0),
        }
    }
}

pub struct Tuner {
    params: &'static Params,
    sample_rate_hz: u32,
}

impl Tuner {
    pub fn new(params: &'static Params, sample_rate_hz: u32) -> Self {
        Self {
            params,
            sample_rate_hz,
        }
    }
}

impl Generator for Tuner {
    fn apply(&mut self, settings: &Settings) {
        let tuning = Tuning::new(settings, self.sample_rate_hz);
        self.params.step.store(tuning.step, Ordering::Relaxed);
        self.params
            .waveform
            .store(tuning.waveform as u8, Ordering::Relaxed);
        self.params
            .amplitude
            .store(tuning.amplitude, Ordering::Relaxed);
    }
}

pub fn run(mut dac: Dac, params: &Params, sine: &SineTable) -> ! {
    let mut phase = 0u32;
    let mut written_amplitude = None;
    loop {
        phase = phase.wrapping_add(params.step.load(Ordering::Relaxed));
        let amplitude = params.amplitude.load(Ordering::Relaxed);
        let word = if written_amplitude == Some(amplitude) {
            let waveform = Waveform::from_u8(params.waveform.load(Ordering::Relaxed));
            dds::channel_a(waveform.sample(phase, sine))
        } else {
            written_amplitude = Some(amplitude);
            dds::channel_b(amplitude)
        };
        dac.write(word);
    }
}
