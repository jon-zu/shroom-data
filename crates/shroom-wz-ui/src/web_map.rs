use std::{
    io::{self, SeekFrom},
    num::{NonZeroU32, NonZeroUsize},
};

use js_sys::Uint8Array;
use lru::LruCache;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Blob;

pub struct MemoryMappedFile {
    blob: Blob,
    size: u64,
    block_size: u32,
    cache: LruCache<usize, Vec<u8>>,
    position: u64,
}

/// Helper to read a blob
async fn read_blob(blob: &Blob, offset: u64, length: u64) -> Result<Vec<u8>, JsValue> {
    let blob_slice = blob.slice_with_f64_and_f64(offset as f64, (offset + length) as f64)?;
    let buf = JsFuture::from(blob_slice.array_buffer()).await?;
    let buf = buf.dyn_into::<js_sys::ArrayBuffer>().expect("array buffer");
    let array = Uint8Array::new(&buf);
    Ok(array.to_vec())
}

impl MemoryMappedFile {
    pub async fn new_with_seed(
        blob: web_sys::Blob,
        seed: u64,
        block_size: NonZeroU32,
        cache_size: NonZeroUsize,
    ) -> Result<MemoryMappedFile, JsValue> {
        let size = blob.size();
        let cache = LruCache::new(cache_size);

        Ok(MemoryMappedFile {
            blob,
            size: size as u64,
            block_size: block_size.into(),
            cache,
            position: seed,
        })
    }

    /// Get the block for the current
    fn get_block_ix(&self, offset: u64) -> usize {
        (offset / self.block_size as u64) as usize
    }

    async fn get(&mut self, offset: u64) -> Result<&[u8], JsValue> {
        let block_ix = self.get_block_ix(offset);

        if !self.cache.contains(&block_ix) {
            let block = read_blob(&self.blob, offset, self.block_size as u64).await?;
            self.cache.put(block_ix, block);
        }

        Ok(self
            .cache
            .get(&block_ix)
            .expect("Block must exist")
            .as_slice())
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, JsValue> {
        if self.position > self.size {
            // TODO: Return EOF here
        }

        // Get the position in the block
        let block_pos = self.position % self.block_size as u64;
        let block_avail = (self.block_size as u64 - block_pos) as usize;
        let n = buf.len().min(block_avail);

        let block = self.get(self.position).await?;
        buf.copy_from_slice(&block[..n]);
        self.position += n as u64;
        Ok(n)
    }
}

impl std::io::Seek for MemoryMappedFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => {
                if offset >= 0 {
                    self.size.checked_add(offset as u64).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "invalid seek to end")
                    })?
                } else {
                    self.size.checked_sub((-offset) as u64).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "invalid seek to end")
                    })?
                }
            }
            SeekFrom::Current(offset) => {
                let current_pos = self.position.checked_add(offset as u64).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid seek from current position",
                    )
                })?;
                if current_pos > self.size {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid seek from current position",
                    ));
                }
                current_pos
            }
        };

        self.position = new_pos;
        Ok(self.position)
    }
}
