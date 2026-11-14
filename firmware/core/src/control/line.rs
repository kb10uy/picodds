pub(super) struct LineTooLong;

pub(super) struct LineBuffer<const N: usize> {
    buf: [u8; N],
    len: usize,
    overflowed: bool,
}

impl<const N: usize> LineBuffer<N> {
    pub(super) const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
            overflowed: false,
        }
    }

    pub(super) fn push(&mut self, byte: u8) {
        match self.buf.get_mut(self.len) {
            Some(slot) if !self.overflowed => {
                *slot = byte;
                self.len += 1;
            }
            _ => self.overflowed = true,
        }
    }

    pub(super) fn backspace(&mut self) {
        if !self.overflowed {
            self.len = self.len.saturating_sub(1);
        }
    }

    pub(super) fn contents(&self) -> Result<&[u8], LineTooLong> {
        if self.overflowed {
            Err(LineTooLong)
        } else {
            Ok(&self.buf[..self.len])
        }
    }

    pub(super) fn clear(&mut self) {
        self.len = 0;
        self.overflowed = false;
    }
}
