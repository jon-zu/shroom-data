pub const WZ_IV_LEN: usize = 16;
pub type WzIv = [u8; WZ_IV_LEN];
pub type WzAesKey = [u8; 32];

pub const GMS_WZ_IV: &WzIv =  include_bytes!("../keys/gms_iv.bin");
pub const SEA_WZ_IV: &WzIv = include_bytes!("../keys/sea_iv.bin");
pub const DEFAULT_WZ_IV: &WzIv = include_bytes!("../keys/default_iv.bin");
pub const WZ_AES_KEY: &WzAesKey = include_bytes!("../keys/aes.bin");
pub const WZ_OFFSET_MAGIC: u32 = u32::from_be_bytes(*include_bytes!("../keys/wz_magic.bin"));

#[derive(Debug)]
pub struct WzCryptoContext {
    pub initial_iv: WzIv,
    pub key: WzAesKey,
    pub offset_magic: u32
}


pub const GMS_CRYPTO_CTX: &WzCryptoContext = &WzCryptoContext{
    initial_iv: *GMS_WZ_IV,
    key: *WZ_AES_KEY,
    offset_magic: WZ_OFFSET_MAGIC,
};

pub const SEA_CRYPTO_CTX: &WzCryptoContext = &WzCryptoContext{
    initial_iv: *SEA_WZ_IV,
    key: *WZ_AES_KEY,
    offset_magic: WZ_OFFSET_MAGIC,
};

pub const DEFAULT_CRYPTO_CTX: &WzCryptoContext = &WzCryptoContext{
    initial_iv: *DEFAULT_WZ_IV,
    key: *WZ_AES_KEY,
    offset_magic: WZ_OFFSET_MAGIC,
};