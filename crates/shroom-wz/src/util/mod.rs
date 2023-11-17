use crate::crypto::WzCrypto;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

pub mod animation;

pub fn custom_binrw_error<R: std::io::Read + std::io::Seek>(
    r: &mut R,
    err: anyhow::Error,
) -> binrw::Error {
    binrw::Error::Custom {
        pos: r.stream_position().unwrap_or(0),
        err: Box::new(err),
    }
}

pub fn wz_checksum(seed: i32, data: &[u8]) -> i32 {
    data.iter()
        .fold(seed, |acc, &b| acc.overflowing_add(b as i32).0)
}

pub trait PeekExt: BufRead + Seek {
    fn peek_n<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let buf = self.fill_buf()?;

        if buf.len() < N {
            let pos = self.stream_position()?;
            let mut buf = [0u8; N];
            self.read_exact(&mut buf)?;
            self.seek(SeekFrom::Start(pos))?;
            return Ok(buf);
        }

        Ok(buf[..N].try_into().unwrap())
    }

    fn peek_u16(&mut self) -> io::Result<u16> {
        self.peek_n::<2>().map(u16::from_le_bytes)
    }
}

impl<T: BufRead + Seek> PeekExt for T {}

pub trait BufReadExt: BufRead {
    fn wz_checksum(&mut self, mut n: u64) -> io::Result<i32> {
        let mut checksum: i32 = 0;

        while n > 0 {
            let buf = self.fill_buf()?;
            if buf.is_empty() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "checksum"));
            }

            let ln = buf.len().min(n as usize);
            checksum = wz_checksum(checksum, &buf[..ln]);
            n -= ln as u64;
            self.consume(ln)
        }

        Ok(checksum)
    }

    fn read_u32(&mut self) -> io::Result<u32> {
        self.read_n().map(u32::from_le_bytes)
    }

    fn read_n<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut buf = [0; N];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_chunked_data(&mut self, crypto: &WzCrypto, chunked_len: usize) -> io::Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(chunked_len);
        // Read chunks
        let mut i = 0;
        while i < chunked_len {
            let chunk_size = self.read_u32()? as usize;
            i += 4;

            if chunk_size > chunked_len {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Bad chunk size {chunk_size}, max: {chunked_len}"),
                ));
            }
            let n = buf.len();
            buf.resize(n + chunk_size, 0);

            let (_, tail) = buf.split_at_mut(n);
            self.read_exact(tail)?;
            crypto.transform(tail.into());
            i += chunk_size
        }

        Ok(buf)
    }
    /*
    fn decompress_flate(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        flate2::bufread::ZlibDecoder::new(self).read_to_end(buf)
    }*/

    fn decompress_flate_size(&mut self, buf: &mut Vec<u8>, size: usize) -> io::Result<usize> {
        buf.resize(size, 0);
        flate2::bufread::ZlibDecoder::new(self).read_exact(buf)?;
        Ok(size)
        //self.decompress_flate(buf)
    }
}

impl<T: BufRead> BufReadExt for T {}

pub trait WriteExt: Write {
    fn write_u32(&mut self, n: u32) -> io::Result<()> {
        self.write_all(n.to_le_bytes().as_slice())
    }

    fn write_chunked_data<'a>(
        &mut self,
        crypto: &WzCrypto,
        chunks: impl Iterator<Item = &'a mut [u8]>,
    ) -> anyhow::Result<usize> {
        let mut written = 0;

        for chunk in chunks {
            self.write_u32(chunk.len() as u32)?;
            crypto.transform(chunk.into());
            self.write_all(chunk)?;
            written += 4 + chunk.len()
        }

        Ok(written)
    }

    fn compress_flate(&mut self, data: &[u8]) -> io::Result<u64> {
        let mut enc = flate2::write::ZlibEncoder::new(self, flate2::Compression::best());
        enc.write_all(data)?;
        enc.try_finish()?;
        Ok(enc.total_out())
    }
}

impl<T: Write> WriteExt for T {}

pub struct SubReader<'a, R> {
    inner: &'a mut R,
    offset: u64,
    size: u64,
}

impl<'a, R> Read for SubReader<'a, R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a, R> BufRead for SubReader<'a, R>
where
    R: BufRead,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }
}

// TODO this MUST be tested
impl<'a, R> Seek for SubReader<'a, R>
where
    R: Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let pos = match pos {
            SeekFrom::Current(p) => SeekFrom::Current(p),
            SeekFrom::End(p) => SeekFrom::End((self.offset + self.size) as i64 + p),
            SeekFrom::Start(p) => SeekFrom::Start(p + self.offset),
        };
        self.inner.seek(pos).map(|p| p - self.offset)
    }
}

impl<'a, R> SubReader<'a, R>
where
    R: Read + Seek,
{
    pub fn new(r: &'a mut R, offset: u64, size: u64) -> Self {
        Self {
            inner: r,
            offset,
            size,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use crate::GMS95;

    use super::*;

    #[test]
    fn read_peek() {
        let data = [0x1, 0x2, 0x3, 0x4];

        let mut r = BufReader::new(Cursor::new(data));
        assert_eq!(r.peek_n::<2>().unwrap(), [1, 2]);
        assert!(r.peek_n::<5>().is_err());

        assert_eq!(r.read_n().unwrap(), [1, 2]);
        assert_eq!(r.peek_n::<2>().unwrap(), [3, 4]);
        assert_eq!(r.read_n().unwrap(), [3, 4]);
        assert!(r.peek_n::<1>().is_err());
    }

    #[test]
    fn chunked() {
        let mut rw = Cursor::new(Vec::new());
        let crypto = WzCrypto::from_cfg(GMS95, 1337);

        let mut data = [0xff; 4096];

        // Write chunks
        rw.write_chunked_data(&crypto, data.chunks_mut(128))
            .unwrap();
        rw.set_position(0);

        // Check buffer len
        assert_eq!(rw.get_ref().len(), 4096 + (4096 / 128) * 4);

        // Read chunks back
        let read = rw.read_chunked_data(&crypto, data.len()).unwrap();
        assert!(read.iter().all(|c| *c == 0xff));
    }

    #[test]
    fn checksum() {
        const N: usize = 4096 * 2 + 3;
        let data = [0x1; N];
        let mut r = Cursor::new(&data);

        assert_eq!(r.wz_checksum(N as u64).unwrap(), N as i32);
    }
}
