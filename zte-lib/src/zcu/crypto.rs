use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use aes::cipher::block_padding::Pkcs7;

use super::constants::*;

type Aes128EcbDec = ecb::Decryptor<aes::Aes128>;
type Aes128EcbEnc = ecb::Encryptor<aes::Aes128>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

/// Pad or truncate key to the required size.
fn fix_key(key: &[u8], size: usize) -> Vec<u8> {
    let mut k = key.to_vec();
    k.resize(size, 0);
    k.truncate(size);
    k
}

/// AES-128-ECB decrypt with PKCS7 unpadding (fallback to raw on bad padding).
pub fn ecb_decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    let key = fix_key(key, AES_128_KEY_SIZE);
    let key_arr: [u8; 16] = key.try_into().unwrap();

    let mut buf = data.to_vec();
    match Aes128EcbDec::new(&key_arr.into()).decrypt_padded_mut::<Pkcs7>(&mut buf) {
        Ok(pt) => Ok(pt.to_vec()),
        Err(_) => {
            // Fallback: decrypt without unpadding
            let mut buf2 = data.to_vec();
            let decryptor = Aes128EcbDec::new(&key_arr.into());
            for chunk in buf2.chunks_exact_mut(AES_BLOCK_SIZE) {
                decryptor.clone().decrypt_block_mut(chunk.into());
            }
            Ok(buf2)
        }
    }
}

/// AES-128-ECB encrypt with PKCS7 padding.
pub fn ecb_encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    let key = fix_key(key, AES_128_KEY_SIZE);
    let key_arr: [u8; 16] = key.try_into().unwrap();
    Aes128EcbEnc::new(&key_arr.into())
        .encrypt_padded_vec_mut::<Pkcs7>(data)
}

/// AES-256-CBC decrypt. First 16 bytes of data are the IV.
pub fn cbc_decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() < AES_CBC_IV_SIZE + AES_BLOCK_SIZE {
        return Err("Data too short for CBC decryption".into());
    }
    let key = fix_key(key, AES_256_KEY_SIZE);
    let key_arr: [u8; 32] = key.try_into().unwrap();
    let iv: [u8; 16] = data[..AES_CBC_IV_SIZE].try_into().unwrap();
    let ciphertext = &data[AES_CBC_IV_SIZE..];

    let mut buf = ciphertext.to_vec();
    match Aes256CbcDec::new(&key_arr.into(), &iv.into()).decrypt_padded_mut::<Pkcs7>(&mut buf)
    {
        Ok(pt) => Ok(pt.to_vec()),
        Err(_) => {
            // Fallback: decrypt without unpadding
            let mut buf2 = ciphertext.to_vec();
            // Ensure length is block-aligned
            let aligned = buf2.len() - (buf2.len() % AES_BLOCK_SIZE);
            buf2.truncate(aligned);
            if !buf2.is_empty() {
                Aes256CbcDec::new(&key_arr.into(), &iv.into())
                    .decrypt_padded_mut::<Pkcs7>(&mut buf2)
                    .ok();
            }
            Ok(buf2)
        }
    }
}

/// AES-256-CBC encrypt. Returns IV (16 bytes) + ciphertext.
pub fn cbc_encrypt(data: &[u8], key: &[u8], iv: Option<&[u8; 16]>) -> Vec<u8> {
    let key = fix_key(key, AES_256_KEY_SIZE);
    let key_arr: [u8; 32] = key.try_into().unwrap();
    let iv_bytes = match iv {
        Some(iv) => *iv,
        None => {
            let mut buf = [0u8; 16];
            getrandom(&mut buf);
            buf
        }
    };
    let ciphertext =
        Aes256CbcEnc::new(&key_arr.into(), &iv_bytes.into()).encrypt_padded_vec_mut::<Pkcs7>(data);
    let mut result = iv_bytes.to_vec();
    result.extend_from_slice(&ciphertext);
    result
}

fn getrandom(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(buf);
    }
}
