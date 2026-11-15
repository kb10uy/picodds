#![no_std]
#![no_main]

mod control;
mod dac;
mod generator;
mod store;

use core::panic::PanicInfo;
use dac::Dac;
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_rp::{
    bind_interrupts,
    multicore::{Stack, spawn_core1},
    peripherals::{PIO0, USB},
    pio::{self, Pio},
    usb,
};
use embassy_usb::{
    Builder, Config,
    class::cdc_acm::{CdcAcmClass, State},
};
use generator::{Params, Tuner};
use picodds_core::{control::Generator, custom::Bank, settings::Settings, waveform::SineTable};
use store::Store;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

const USB_VID: u16 = 0x1209;
const USB_PID: u16 = 0x0010;
const CORE1_STACK_BYTES: usize = 4096;

static PARAMS: Params = Params::new();
static BANK: Bank = Bank::new();

#[unsafe(link_section = ".data")]
static SINE: SineTable = SineTable::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut store = Store::new(p.FLASH);
    let _ = BANK.load(&mut store);

    let dac = Dac::new(Pio::new(p.PIO0, Irqs), p.PIN_6, p.PIN_7, p.PIN_29);
    let settings = Settings::default();
    let mut tuner = Tuner::new(&PARAMS, dac.sample_rate_hz());
    tuner.apply(&settings);
    let stack = cortex_m::singleton!(: Stack<CORE1_STACK_BYTES> = Stack::new()).unwrap();
    spawn_core1(p.CORE1, stack, move || {
        generator::run(dac, &PARAMS, &SINE, &BANK)
    });

    let mut usb_config = Config::new(USB_VID, USB_PID);
    usb_config.product = Some("picodds");

    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut cdc_state = State::new();
    let mut builder = Builder::new(
        usb::Driver::new(p.USB, Irqs),
        usb_config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [],
        &mut control_buf,
    );
    let control_port = CdcAcmClass::new(&mut builder, &mut cdc_state, control::MAX_PACKET_SIZE);
    let mut usb_device = builder.build();

    join(
        usb_device.run(),
        control::run(control_port, settings, &BANK, tuner, store),
    )
    .await;
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    loop {
        cortex_m::asm::nop();
    }
}
