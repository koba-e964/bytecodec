//! I/O (i.e., `Read` and `Write` traits) related module.
pub use crate::io_async::{ReadBuf, WriteBuf};
use crate::{ByteCount, Decode, Encode, Eos, Error, ErrorKind, Result};
use std::cmp;
use std::io::{Read, Write};

/// An extension of `Decode` trait to aid decodings involving I/O.
pub trait IoDecodeExt: Decode {
    /// Consumes bytes from the given read buffer and proceeds the decoding process.
    fn decode_from_read_buf<B>(&mut self, buf: &mut ReadBuf<B>) -> Result<()>
    where
        B: AsRef<[u8]>,
    {
        let eos = Eos::new(buf.stream_state.is_eos());
        let size = track!(self.decode(&buf.inner.as_ref()[buf.head..buf.tail], eos))?;
        buf.head += size;
        if buf.head == buf.tail {
            buf.head = 0;
            buf.tail = 0;
        }
        Ok(())
    }

    /// Decodes an item from the given reader.
    ///
    /// This method reads only minimal bytes required to decode an item.
    ///
    /// Note that this is a blocking method.
    fn decode_exact<R: Read>(&mut self, mut reader: R) -> Result<Self::Item> {
        let mut buf = [0; 1024];
        loop {
            let mut size = match self.requiring_bytes() {
                ByteCount::Finite(n) => cmp::min(n, buf.len() as u64) as usize,
                ByteCount::Infinite => buf.len(),
                ByteCount::Unknown => 1,
            };
            let eos = if size != 0 {
                size = track!(reader.read(&mut buf[..size]).map_err(Error::from))?;
                Eos::new(size == 0)
            } else {
                Eos::new(false)
            };

            let consumed = track!(self.decode(&buf[..size], eos))?;
            track_assert_eq!(consumed, size, ErrorKind::InconsistentState; self.is_idle(), eos);
            if self.is_idle() {
                let item = track!(self.finish_decoding())?;
                return Ok(item);
            }
        }
    }
}
impl<T: Decode> IoDecodeExt for T {}

/// An extension of `Encode` trait to aid encodings involving I/O.
pub trait IoEncodeExt: Encode {
    /// Encodes the items remaining in the encoder and
    /// writes the encoded bytes to the given write buffer.
    fn encode_to_write_buf<B>(&mut self, buf: &mut WriteBuf<B>) -> Result<()>
    where
        B: AsMut<[u8]>,
    {
        let eos = Eos::new(buf.stream_state.is_eos());
        let size = track!(self.encode(&mut buf.inner.as_mut()[buf.tail..], eos))?;
        buf.tail += size;
        Ok(())
    }

    /// Encodes all of the items remaining in the encoder and
    /// writes the encoded bytes to the given writer.
    ///
    /// Note that this is a blocking method.
    fn encode_all<W: Write>(&mut self, mut writer: W) -> Result<()> {
        let mut buf = [0; 1024];
        while !self.is_idle() {
            let size = track!(self.encode(&mut buf[..], Eos::new(false)))?;
            track!(writer.write_all(&buf[..size]).map_err(Error::from))?;
            if !self.is_idle() {
                track_assert_ne!(size, 0, ErrorKind::Other);
            }
        }
        Ok(())
    }
}
impl<T: Encode> IoEncodeExt for T {}

/// State of I/O streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum StreamState {
    Normal,
    Eos,
    WouldBlock,
    Error,
}
impl StreamState {
    /// Returns `true` if the state is `Normal`, otherwise `false`.
    pub fn is_normal(self) -> bool {
        self == StreamState::Normal
    }

    /// Returns `true` if the state is `Error`, otherwise `false`.
    pub fn is_error(self) -> bool {
        self == StreamState::Error
    }

    /// Returns `true` if the state is `Eos`, otherwise `false`.
    pub fn is_eos(self) -> bool {
        self == StreamState::Eos
    }

    /// Returns `true` if the state is `WouldBlock`, otherwise `false`.
    pub fn would_block(self) -> bool {
        self == StreamState::WouldBlock
    }
}

/// Buffered I/O stream.
#[derive(Debug)]
pub struct BufferedIo<T> {
    stream: T,
    rbuf: ReadBuf<Vec<u8>>,
    wbuf: WriteBuf<Vec<u8>>,
}
impl<T: Read + Write> BufferedIo<T> {
    /// Makes a new `BufferedIo` instance.
    pub fn new(stream: T, read_buf_size: usize, write_buf_size: usize) -> Self {
        BufferedIo {
            stream,
            rbuf: ReadBuf::new(vec![0; read_buf_size]),
            wbuf: WriteBuf::new(vec![0; write_buf_size]),
        }
    }

    /// Executes an I/O operation on the inner stream.
    ///
    /// "I/O operation" means "filling the read buffer" and "flushing the write buffer".
    pub fn execute_io(&mut self) -> Result<()> {
        track!(self.rbuf.fill(&mut self.stream))?;
        track!(self.wbuf.flush(&mut self.stream))?;
        Ok(())
    }

    /// Returns `true` if the inner stream reaches EOS, otherwise `false`.
    pub fn is_eos(&self) -> bool {
        self.rbuf.stream_state().is_eos() || self.wbuf.stream_state().is_eos()
    }

    /// Returns `true` if the previous I/O operation on the inner stream would block, otherwise `false`.
    pub fn would_block(&self) -> bool {
        self.rbuf.stream_state().would_block()
            && (self.wbuf.is_empty() || self.wbuf.stream_state().would_block())
    }

    /// Returns a reference to the read buffer of the instance.
    pub fn read_buf_ref(&self) -> &ReadBuf<Vec<u8>> {
        &self.rbuf
    }

    /// Returns a mutable reference to the read buffer of the instance.
    pub fn read_buf_mut(&mut self) -> &mut ReadBuf<Vec<u8>> {
        &mut self.rbuf
    }

    /// Returns a reference to the write buffer of the instance.
    pub fn write_buf_ref(&self) -> &WriteBuf<Vec<u8>> {
        &self.wbuf
    }

    /// Returns a mutable reference to the write buffer of the instance.
    pub fn write_buf_mut(&mut self) -> &mut WriteBuf<Vec<u8>> {
        &mut self.wbuf
    }

    /// Returns a reference to the inner stream of the instance.
    pub fn stream_ref(&self) -> &T {
        &self.stream
    }

    /// Returns a mutable reference to the inner stream of the instance.
    pub fn stream_mut(&mut self) -> &mut T {
        &mut self.stream
    }

    /// Takes ownership of the instance, and returns the inner stream.
    pub fn into_stream(self) -> T {
        self.stream
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::bytes::{Utf8Decoder, Utf8Encoder};
    use crate::EncodeExt;
    use std::io::{Read, Write};

    #[test]
    fn decode_from_read_buf_works() {
        let mut buf = ReadBuf::new(vec![0; 1024]);
        track_try_unwrap!(buf.fill(b"foo".as_ref()));
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.stream_state(), StreamState::Eos);

        let mut decoder = Utf8Decoder::new();
        track_try_unwrap!(decoder.decode_from_read_buf(&mut buf));
        assert_eq!(track_try_unwrap!(decoder.finish_decoding()), "foo");
    }

    #[test]
    fn read_from_read_buf_works() {
        let mut rbuf = ReadBuf::new(vec![0; 1024]);
        track_try_unwrap!(rbuf.fill(b"foo".as_ref()));
        assert_eq!(rbuf.len(), 3);
        assert_eq!(rbuf.stream_state(), StreamState::Eos);

        let mut buf = Vec::new();
        rbuf.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"foo");
        assert_eq!(rbuf.len(), 0);
    }

    #[test]
    fn encode_to_write_buf_works() {
        let mut encoder = track_try_unwrap!(Utf8Encoder::with_item("foo"));

        let mut buf = WriteBuf::new(vec![0; 1024]);
        track_try_unwrap!(encoder.encode_to_write_buf(&mut buf));
        assert_eq!(buf.len(), 3);

        let mut v = Vec::new();
        track_try_unwrap!(buf.flush(&mut v));
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.stream_state(), StreamState::Normal);
        assert_eq!(v, b"foo");
    }

    #[test]
    fn write_to_write_buf_works() {
        let mut buf = WriteBuf::new(vec![0; 1024]);
        buf.write_all(b"foo").unwrap();
        assert_eq!(buf.len(), 3);

        let mut v = Vec::new();
        track_try_unwrap!(buf.flush(&mut v));
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.stream_state(), StreamState::Normal);
        assert_eq!(v, b"foo");
    }
}
