/// ZTE config file header magic bytes.
pub const HEADER_MAGIC: &[u8; 4] = b"ZXHN";

/// Payload type identifiers (from header at offset 0x04).
pub const PAYLOAD_TYPE_ECB: u32 = 0;
pub const PAYLOAD_TYPE_CBC: u32 = 1;
pub const PAYLOAD_TYPE_PLAIN: u32 = 2;
pub const PAYLOAD_TYPE_AESCBC_NEW: u32 = 3;

/// Header layout.
pub const HEADER_SIZE: usize = 128;
pub const SIGNATURE_OFFSET: usize = 8;
pub const SIGNATURE_MAX_LEN: usize = 64;
pub const PAYLOAD_TYPE_OFFSET: usize = 4;
pub const PAYLOAD_OFFSET_FIELD: usize = 72;

/// ZLIB chunk format.
pub const ZLIB_CHUNK_HEADER_SIZE: usize = 4;

/// AES block sizes.
pub const AES_BLOCK_SIZE: usize = 16;
pub const AES_128_KEY_SIZE: usize = 16;
pub const AES_256_KEY_SIZE: usize = 32;
pub const AES_CBC_IV_SIZE: usize = 16;
