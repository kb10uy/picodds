use embassy_rp::{
    Peri,
    clocks::clk_sys_freq,
    gpio::Level,
    peripherals::PIO0,
    pio::{
        Config, Direction, FifoJoin, Pio, PioPin, ShiftConfig, ShiftDirection, StateMachine,
        program::pio_asm,
    },
};

const TARGET_SAMPLE_RATE_HZ: u32 = 500_000;
const CYCLES_PER_WORD: u32 = 50;

pub struct Dac {
    sm: StateMachine<'static, PIO0, 0>,
    sample_rate_hz: u32,
}

impl Dac {
    pub fn new(
        pio: Pio<'static, PIO0>,
        sck: Peri<'static, impl PioPin>,
        cs: Peri<'static, impl PioPin>,
        sdi: Peri<'static, impl PioPin>,
    ) -> Self {
        let Pio {
            mut common,
            mut sm0,
            ..
        } = pio;
        let program = pio_asm!(
            ".side_set 2",
            ".wrap_target",
            "    pull block      side 0b10 [7]",
            "    nop             side 0b10 [7]",
            "    set x, 15       side 0b00 [1]",
            "bit:",
            "    out pins, 1     side 0b00",
            "    jmp x-- bit     side 0b01",
            ".wrap",
        );
        let program = common.load_program(&program.program);
        let sck = common.make_pio_pin(sck);
        let cs = common.make_pio_pin(cs);
        let sdi = common.make_pio_pin(sdi);
        sm0.set_pins(Level::High, &[&cs]);
        sm0.set_pins(Level::Low, &[&sck, &sdi]);
        sm0.set_pin_dirs(Direction::Out, &[&sck, &cs, &sdi]);

        let clock_hz = clk_sys_freq();
        let divider = clock_hz / (TARGET_SAMPLE_RATE_HZ * CYCLES_PER_WORD);
        let mut config = Config::default();
        config.use_program(&program, &[&sck, &cs]);
        config.set_out_pins(&[&sdi]);
        config.shift_out = ShiftConfig {
            threshold: 32,
            direction: ShiftDirection::Left,
            auto_fill: false,
        };
        config.fifo_join = FifoJoin::TxOnly;
        config.clock_divider = u8::try_from(divider).unwrap().into();
        sm0.set_config(&config);
        sm0.set_enable(true);

        Self {
            sm: sm0,
            sample_rate_hz: clock_hz / (divider * CYCLES_PER_WORD),
        }
    }

    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    pub fn write(&mut self, word: u16) {
        while !self.sm.tx().try_push(u32::from(word) << 16) {}
    }
}
