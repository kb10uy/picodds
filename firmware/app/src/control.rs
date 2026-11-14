use crate::generator::Tuner;
use core::fmt;
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_time::Timer;
use embassy_usb::{
    class::cdc_acm::CdcAcmClass,
    driver::{Driver, EndpointError},
};
use picodds_core::{
    control::{Action, Controller},
    settings::Settings,
};

pub const MAX_PACKET_SIZE: u16 = 64;

const LINE_BYTES: usize = 64;
const REPLY_BYTES: usize = 128;

pub async fn run<'d, D: Driver<'d>>(
    mut class: CdcAcmClass<'d, D>,
    settings: Settings,
    mut tuner: Tuner,
) -> ! {
    let mut controller = Controller::<LINE_BYTES>::new(settings, env!("CARGO_PKG_VERSION"));
    let mut packet = [0; MAX_PACKET_SIZE as usize];
    let mut reply = Reply::new();
    loop {
        class.wait_connection().await;
        while let Ok(len) = class.read_packet(&mut packet).await {
            for &byte in &packet[..len] {
                reply.clear();
                let action = controller.feed(byte, &mut tuner, &mut reply);
                if let Some(line) = reply.line() {
                    let _ = write_line(&mut class, line).await;
                }
                if action == Some(Action::Bootsel) {
                    Timer::after_millis(50).await;
                    reset_to_usb_boot(0, 0);
                }
            }
        }
    }
}

async fn write_line<'d, D: Driver<'d>>(
    class: &mut CdcAcmClass<'d, D>,
    line: &[u8],
) -> Result<(), EndpointError> {
    let max = usize::from(MAX_PACKET_SIZE);
    for chunk in line.chunks(max) {
        class.write_packet(chunk).await?;
    }
    if line.len().is_multiple_of(max) {
        class.write_packet(&[]).await?;
    }
    Ok(())
}

struct Reply {
    buf: [u8; REPLY_BYTES],
    len: usize,
    overflowed: bool,
}

impl Reply {
    const fn new() -> Self {
        Self {
            buf: [0; REPLY_BYTES],
            len: 0,
            overflowed: false,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.overflowed = false;
    }

    fn line(&self) -> Option<&[u8]> {
        (!self.overflowed && self.len > 0).then(|| &self.buf[..self.len])
    }
}

impl fmt::Write for Reply {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        let Some(dest) = self.buf.get_mut(self.len..end) else {
            self.overflowed = true;
            return Err(fmt::Error);
        };
        dest.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}
