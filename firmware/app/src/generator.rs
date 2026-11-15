use crate::dac::Dac;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, Ordering};
use picodds_core::{
    control::Generator,
    custom::Bank,
    dds::{self, Tuning},
    settings::Settings,
    waveform::{MIDDLE, SineTable, Waveform},
};

pub struct Params {
    step: AtomicU32,
    waveform: AtomicU8,
    amplitude: AtomicU16,
    mute: AtomicBool,
    muted: AtomicBool,
}

impl Params {
    pub const fn new() -> Self {
        Self {
            step: AtomicU32::new(0),
            waveform: AtomicU8::new(0),
            amplitude: AtomicU16::new(0),
            mute: AtomicBool::new(false),
            muted: AtomicBool::new(false),
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

    fn wait_muted(&self, muted: bool) {
        while self.params.muted.load(Ordering::Acquire) != muted {
            core::hint::spin_loop();
        }
    }
}

impl Generator for Tuner {
    fn apply(&mut self, settings: &Settings) {
        let tuning = Tuning::new(settings, self.sample_rate_hz);
        self.params.step.store(tuning.step, Ordering::Relaxed);
        self.params
            .waveform
            .store(tuning.waveform.to_u8(), Ordering::Relaxed);
        self.params
            .amplitude
            .store(tuning.amplitude, Ordering::Relaxed);
    }

    fn mute(&mut self) {
        self.params.mute.store(true, Ordering::Release);
        self.wait_muted(true);
    }

    fn unmute(&mut self) {
        self.params.mute.store(false, Ordering::Release);
        self.wait_muted(false);
    }
}

pub fn run(mut dac: Dac, params: &Params, sine: &SineTable, bank: &Bank) -> ! {
    let mut phase = 0u32;
    let mut written_amplitude = None;
    loop {
        if params.mute.load(Ordering::Acquire) {
            dac.write(dds::channel_a(MIDDLE));
            params.muted.store(true, Ordering::Release);
            continue;
        }
        params.muted.store(false, Ordering::Release);
        phase = phase.wrapping_add(params.step.load(Ordering::Relaxed));
        let amplitude = params.amplitude.load(Ordering::Relaxed);
        let word = if written_amplitude == Some(amplitude) {
            let waveform = Waveform::from_u8(params.waveform.load(Ordering::Relaxed));
            dds::channel_a(waveform.sample(phase, sine, bank))
        } else {
            written_amplitude = Some(amplitude);
            dds::channel_b(amplitude)
        };
        dac.write(word);
    }
}
