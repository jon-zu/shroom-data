use crate::keys::{self, WzCryptoContext};

#[derive(Debug, Clone, Copy)]
pub struct WzVersion(pub u16);

impl From<u16> for WzVersion {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<usize> for WzVersion {
    fn from(value: usize) -> Self {
        Self(value as u16)
    }
}

fn version_hash(v: u16) -> u32 {
    let mut buffer = itoa::Buffer::new();
    buffer.format(v).as_bytes().iter().fold(0, |mut acc, &c| {
        acc <<= 5;
        acc + (c as u32) + 1
    })
}

fn encrypt_version(hash: u32) -> u16 {
    (0..4)
        .rev()
        .fold(0xFFu32, |acc, i| acc ^ hash >> (i * 8) & 0xFF) as u16
}

impl WzVersion {
    pub fn hash(&self) -> u32 {
        version_hash(self.0)
    }

    pub fn encrypted_version(&self) -> u16 {
        encrypt_version(self.hash())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum WzRegion {
    GMS,
    SEA,
    Other,
    BmsSrv,
}

impl WzRegion {
    pub fn crypto_context(&self) -> &'static WzCryptoContext {
        match self {
            WzRegion::GMS => keys::GMS_CRYPTO_CTX,
            WzRegion::SEA => keys::SEA_CRYPTO_CTX,
            WzRegion::Other => keys::DEFAULT_CRYPTO_CTX,
            WzRegion::BmsSrv => keys::DEFAULT_CRYPTO_CTX,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::version::WzVersion;

    #[test]
    fn version_hash() {
        let v95 = WzVersion(95);

        assert_eq!(v95.hash(), 1910);
        assert_eq!(v95.encrypted_version(), 142);
    }
}
