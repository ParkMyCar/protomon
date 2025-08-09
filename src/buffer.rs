pub trait WriteBuffer {
    const MIN_SIZE: usize;

    /// Write a single byte into the buffer.
    fn write(&mut self, byte: u8);

    /// Reserver at least `amt` bytes.
    fn reserve(&mut self, amt: usize);
}
