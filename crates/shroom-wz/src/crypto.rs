use std::num::Wrapping;

use aes::cipher::{
    inout::InOutBuf,
    typenum::{U16, U256},
    BlockEncrypt, KeyInit,
};

use crate::{
    keys::{WzCryptoContext, WzIv, WZ_IV_LEN},
    version::WzVersion,
    WzConfig,
};

// Must be a multiple of WZ_IV_LEN
pub const WZ_KEY_BUFFER_LEN: usize = WZ_IV_LEN * 256; // 256
pub type WzKeyBufferLen = U256;

// https://github.com/rust-lang/rust/pull/109049
pub fn as_chunks_mut<const N: usize, const M: usize>(arr: &mut [u8; N]) -> &mut [[u8; M]] {
    assert_ne!(N, 0);
    assert_eq!(N % M, 0);

    let len = N / M;
    let array_slice: &mut [[u8; M]] =
        // SAFETY: We cast a slice of `len * N` elements into
        // a slice of `len` many `N` elements chunks.
        unsafe { std::slice::from_raw_parts_mut(arr.as_mut_ptr().cast(), len) };
    array_slice
}

#[derive(Debug, Clone)]
pub struct WzCrypto {
    cipher: aes::Aes256,
    iv: WzIv,
    version_hash: u32,
    xor_key_buffer: [u8; WZ_KEY_BUFFER_LEN],
    data_offset: u32,
    offset_magic: u32,
}

impl WzCrypto {
    pub fn new(ctx: &WzCryptoContext, version: WzVersion, data_offset: u32) -> Self {
        let cipher = aes::Aes256::new(ctx.key.as_ref().into());

        let mut result = Self {
            iv: ctx.initial_iv,
            cipher,
            xor_key_buffer: [0; WZ_KEY_BUFFER_LEN],
            version_hash: version.hash(),
            data_offset,
            offset_magic: ctx.offset_magic,
        };

        let mut key = [0; WZ_KEY_BUFFER_LEN];
        result.fill_key(&mut key);
        result.xor_key_buffer = key;
        result
    }

    pub fn from_cfg(cfg: WzConfig, data_offset: u32) -> Self {
        Self::new(cfg.region.crypto_context(), cfg.version, data_offset)
    }

    fn fill_key<const N: usize>(&self, key: &mut [u8; N]) {
        assert!(N % WZ_IV_LEN == 0);
        let mut cur_key = self.iv;

        let chunks = as_chunks_mut::<N, WZ_IV_LEN>(key);
        for chunk in chunks {
            self.next_xor_key(&mut cur_key);
            chunk.copy_from_slice(&cur_key);
        }
    }

    fn next_xor_key(&self, key: &mut [u8; 16]) {
        self.cipher.encrypt_block(key.as_mut().into())
    }

    fn transform_small(&self, mut buf: InOutBuf<'_, '_, u8>) {
        let n = buf.len();
        buf.xor_in2out(&self.xor_key_buffer[..n])
    }

    fn transform_large(&self, buf: InOutBuf<'_, '_, u8>) {
        let mut key = self.iv;
        let (chunks, mut tail) = buf.into_chunks::<U16>();

        for mut chunk in chunks {
            self.next_xor_key(&mut key);
            chunk.xor_in2out(&key.into());
        }

        self.next_xor_key(&mut key);
        let n = tail.len();
        tail.xor_in2out(&key[..n]);
    }

    pub fn transform(&self, buf: InOutBuf<'_, '_, u8>) {
        let n = buf.len();
        if n <= WZ_KEY_BUFFER_LEN {
            self.transform_small(buf)
        } else {
            self.transform_large(buf)
        }
    }

    fn offset_key_at(&self, pos: u32, data_offset: u32) -> u32 {
        let mut off = Wrapping(!(pos - data_offset));
        off *= self.version_hash;
        off -= self.offset_magic;

        let off = off.0;
        off.rotate_left(off & 0x1F)
    }

    pub fn decrypt_offset(&self, encrypted_offset: u32, pos: u32) -> u32 {
        let k = self.offset_key_at(pos, self.data_offset);
        (k ^ encrypted_offset).wrapping_add(self.data_offset * 2)
    }

    pub fn encrypt_offset(&self, off: u32, pos: u32) -> u32 {
        let off = off.wrapping_sub(self.data_offset * 2);
        off ^ self.offset_key_at(pos, self.data_offset)
    }

    pub fn offset_link(&self, off: u32) -> u64 {
        self.data_offset as u64 + off as u64
    }
}

#[cfg(test)]
mod tests {
    use crate::GMS95;

    use super::{as_chunks_mut, WzCrypto};

    #[test]
    fn wz_offset() {
        let crypto = WzCrypto::from_cfg(GMS95, 60);

        let c = crypto.encrypt_offset(4681, 89);
        assert_eq!(crypto.decrypt_offset(c, 89), 4681);
    }

    #[test]
    fn chunks() {
        let mut data = [0u8; 4];
        let chunks = as_chunks_mut::<4, 2>(&mut data);

        chunks[0] = [4, 3];
        chunks[1] = [2, 1];

        assert_eq!(data, [4, 3, 2, 1]);
    }

    #[test]
    #[should_panic]
    fn invalid_chunk() {
        let mut data = [0u8; 4];
        as_chunks_mut::<4, 3>(&mut data);
    }

    #[test]
    #[should_panic]
    fn invalid_chunk_empty() {
        let mut data = [0u8; 0];
        as_chunks_mut::<0, 3>(&mut data);
    }
}
