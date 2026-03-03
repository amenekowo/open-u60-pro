use super::compression;
use super::constants::*;
use super::crypto;
use super::keys;
use crate::error::ZteError;

/// Parsed ZXHN config file header.
pub struct ZxhnHeader {
    pub magic: [u8; 4],
    pub payload_type: u32,
    pub signature: Vec<u8>,
    pub payload_offset: usize,
}

/// Parse a ZXHN config file header.
pub fn read_header(data: &[u8]) -> Result<ZxhnHeader, ZteError> {
    if data.len() < HEADER_SIZE {
        return Err(ZteError::Config(format!(
            "File too small ({} bytes) for a valid ZTE config header",
            data.len()
        )));
    }

    let magic: [u8; 4] = data[..4].try_into().unwrap();
    if &magic != HEADER_MAGIC {
        return Err(ZteError::Config(format!(
            "Invalid header magic: {:?} (expected {:?})",
            magic, HEADER_MAGIC
        )));
    }

    let payload_type = u32::from_be_bytes(
        data[PAYLOAD_TYPE_OFFSET..PAYLOAD_TYPE_OFFSET + 4]
            .try_into()
            .unwrap(),
    );

    // Extract null-terminated signature string
    let sig_raw = &data[SIGNATURE_OFFSET..SIGNATURE_OFFSET + SIGNATURE_MAX_LEN];
    let sig_end = sig_raw.iter().position(|&b| b == 0).unwrap_or(sig_raw.len());
    let signature = sig_raw[..sig_end].to_vec();

    let payload_offset = u32::from_be_bytes(
        data[PAYLOAD_OFFSET_FIELD..PAYLOAD_OFFSET_FIELD + 4]
            .try_into()
            .unwrap(),
    ) as usize;
    let payload_offset = if payload_offset == 0 || payload_offset > data.len() {
        HEADER_SIZE
    } else {
        payload_offset
    };

    Ok(ZxhnHeader {
        magic,
        payload_type,
        signature,
        payload_offset,
    })
}

/// Full decryption pipeline: header -> detect -> decrypt -> decompress -> XML.
pub fn decrypt_config(
    data: &[u8],
    key: Option<&[u8]>,
    serial: Option<&str>,
) -> Result<Vec<u8>, ZteError> {
    let header = read_header(data)?;
    let payload = &data[header.payload_offset..];

    if header.payload_type == PAYLOAD_TYPE_PLAIN {
        return compression::decompress(payload)
            .or_else(|_| Ok(payload.to_vec()))
            .map_err(|e: String| ZteError::Config(e));
    }

    let is_cbc = header.payload_type == PAYLOAD_TYPE_CBC
        || header.payload_type == PAYLOAD_TYPE_AESCBC_NEW;

    // If explicit key provided, try it directly
    if let Some(k) = key {
        return try_decrypt(payload, k, is_cbc);
    }

    // Otherwise try all candidate keys
    let sig = if header.signature.is_empty() {
        None
    } else {
        Some(header.signature.as_slice())
    };
    let candidates = keys::get_all_keys(serial, sig);
    let mut last_error = None;
    for (_, candidate_key) in &candidates {
        match try_decrypt(payload, candidate_key, is_cbc) {
            Ok(result) => return Ok(result),
            Err(e) => last_error = Some(e),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        ZteError::Config(
            "Failed to decrypt config with any known key. Try --serial or --key.".into(),
        )
    }))
}

fn try_decrypt(payload: &[u8], key: &[u8], is_cbc: bool) -> Result<Vec<u8>, ZteError> {
    let decrypted = if is_cbc {
        crypto::cbc_decrypt(payload, key).map_err(|e| ZteError::Config(e))?
    } else {
        crypto::ecb_decrypt(payload, key).map_err(|e| ZteError::Config(e))?
    };
    let result = compression::decompress(&decrypted).map_err(|e| ZteError::Config(e))?;

    // Sanity check: decrypted config should be XML
    let stripped = result.iter().position(|&b| !b.is_ascii_whitespace());
    if let Some(pos) = stripped {
        if result[pos] != b'<' {
            return Err(ZteError::Config(
                "Decrypted data does not appear to be XML".into(),
            ));
        }
    }
    Ok(result)
}

/// Full encryption pipeline: compress -> encrypt -> add header.
pub fn encrypt_config(
    xml_data: &[u8],
    key: &[u8],
    payload_type: u32,
    signature: &[u8],
) -> Result<Vec<u8>, ZteError> {
    let compressed =
        compression::compress(xml_data, false, 65536).map_err(|e| ZteError::Config(e))?;

    let encrypted = match payload_type {
        PAYLOAD_TYPE_CBC | PAYLOAD_TYPE_AESCBC_NEW => crypto::cbc_encrypt(&compressed, key, None),
        PAYLOAD_TYPE_PLAIN => compressed,
        _ => crypto::ecb_encrypt(&compressed, key),
    };

    let header = build_header(payload_type, signature);
    let mut result = header;
    result.extend_from_slice(&encrypted);
    Ok(result)
}

fn build_header(payload_type: u32, signature: &[u8]) -> Vec<u8> {
    let mut header = vec![0u8; HEADER_SIZE];
    header[..4].copy_from_slice(HEADER_MAGIC);
    header[PAYLOAD_TYPE_OFFSET..PAYLOAD_TYPE_OFFSET + 4]
        .copy_from_slice(&payload_type.to_be_bytes());

    let sig_len = signature.len().min(SIGNATURE_MAX_LEN);
    header[SIGNATURE_OFFSET..SIGNATURE_OFFSET + sig_len]
        .copy_from_slice(&signature[..sig_len]);

    header[PAYLOAD_OFFSET_FIELD..PAYLOAD_OFFSET_FIELD + 4]
        .copy_from_slice(&(HEADER_SIZE as u32).to_be_bytes());

    header
}
