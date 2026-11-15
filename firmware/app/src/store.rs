use embassy_rp::{
    Peri,
    flash::{Blocking, ERASE_SIZE, Flash},
    peripherals::FLASH,
};
use picodds_core::custom::{FlashError, RECORD_BYTES, SLOTS, SlotFlash};

const FLASH_BYTES: usize = 2 * 1024 * 1024;
const STORE_OFFSET: usize = FLASH_BYTES - SLOTS * ERASE_SIZE;

pub struct Store(Flash<'static, FLASH, Blocking, FLASH_BYTES>);

impl Store {
    pub fn new(flash: Peri<'static, FLASH>) -> Self {
        Self(Flash::new_blocking(flash))
    }
}

fn offset(slot: usize) -> u32 {
    (STORE_OFFSET + slot * ERASE_SIZE) as u32
}

impl SlotFlash for Store {
    fn read(&mut self, slot: usize, record: &mut [u8; RECORD_BYTES]) -> Result<(), FlashError> {
        self.0
            .blocking_read(offset(slot), record)
            .map_err(|_| FlashError)
    }

    fn erase(&mut self, slot: usize) -> Result<(), FlashError> {
        let from = offset(slot);
        self.0
            .blocking_erase(from, from + ERASE_SIZE as u32)
            .map_err(|_| FlashError)
    }

    fn write(&mut self, slot: usize, record: &[u8]) -> Result<(), FlashError> {
        self.0
            .blocking_write(offset(slot), record)
            .map_err(|_| FlashError)
    }
}
