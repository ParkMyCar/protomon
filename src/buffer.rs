pub trait WriteBuffer {
    /// Write a single byte into the buffer.
    fn write(&mut self, byte: u8) -> bool;
    /// Write the slice of bytes into the buffer, returning how many bytes were written.
    fn write_slice(&mut self, slice: &[u8]) -> usize;

    /// Reserve capacity for least `amt` bytes, and how many bytes were newly reserved.
    fn reserve(&mut self, amt: usize) -> usize;

    /// How many bytes are remaining in the buffer.
    fn remaining(&self) -> usize;
}

impl<'a> WriteBuffer for &'a mut [u8] {
    fn write(&mut self, byte: u8) -> bool {
        let len = self.len();
        if len < 1 {
            return false;
        }
        self[0] = byte;
        *self = unsafe {
            let offset_ptr = self.as_mut_ptr().add(1);
            let new_len = len - 1;
            core::slice::from_raw_parts_mut(offset_ptr, new_len)
        };
        true
    }

    fn write_slice(&mut self, slice: &[u8]) -> usize {
        let write_len = self.remaining().min(slice.len());
        self[..write_len].copy_from_slice(&slice[..write_len]);
        *self = unsafe {
            let offset_ptr = self.as_mut_ptr().add(write_len);
            let new_len = self.len() - write_len;
            core::slice::from_raw_parts_mut(offset_ptr, new_len)
        };
        write_len
    }

    fn reserve(&mut self, _amt: usize) -> usize {
        0
    }

    fn remaining(&self) -> usize {
        self.len()
    }
}

impl WriteBuffer for Vec<u8> {
    fn write(&mut self, byte: u8) -> bool {
        self.push(byte);
        true
    }

    fn write_slice(&mut self, slice: &[u8]) -> usize {
        let write_len = self.remaining().min(slice.len());

        let str_write = self.len();
        let end_write = str_write + write_len;
        self[str_write..end_write].copy_from_slice(&slice[..write_len]);
        unsafe {
            self.set_len(end_write);
        }

        write_len
    }

    fn reserve(&mut self, amt: usize) -> usize {
        let og_capacity = self.capacity();
        self.reserve(amt);
        self.capacity() - og_capacity
    }

    fn remaining(&self) -> usize {
        self.capacity() - self.len()
    }
}
