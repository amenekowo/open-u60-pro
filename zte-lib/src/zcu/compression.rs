use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::read::DeflateDecoder;
use flate2::Compression;
use std::io::{Read, Write};

/// Decompress ZTE config payload with multiple fallback strategies.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    // 1. Try plain zlib
    if let Ok(result) = decompress_zlib(data) {
        return Ok(result);
    }

    // 2. Try chunked format
    if let Ok(result) = decompress_chunked(data) {
        return Ok(result);
    }

    // 3. Try raw deflate (no zlib header)
    if let Ok(result) = decompress_deflate(data) {
        return Ok(result);
    }

    // 4. Skip potential garbage header bytes and retry
    for skip in [2, 4, 8, 16] {
        if data.len() > skip {
            if let Ok(result) = decompress_zlib(&data[skip..]) {
                return Ok(result);
            }
        }
    }

    Err("Failed to decompress data: not valid ZLIB or chunked ZLIB format".into())
}

fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|e| e.to_string())?;
    Ok(result)
}

fn decompress_deflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = DeflateDecoder::new(data);
    let mut result = Vec::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|e| e.to_string())?;
    Ok(result)
}

/// Decompress ZTE chunked ZLIB format: repeated [4-byte BE length][zlib data].
fn decompress_chunked(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let mut offset = 0;
    let mut chunks_found = 0;

    while offset < data.len() {
        if offset + 4 > data.len() {
            break;
        }
        let chunk_len =
            u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        if chunk_len == 0 {
            break;
        }
        if chunk_len > data.len() - offset {
            return Err(format!(
                "Chunk length {chunk_len} exceeds remaining data ({} bytes)",
                data.len() - offset
            ));
        }

        let chunk_data = &data[offset..offset + chunk_len];
        offset += chunk_len;

        match decompress_zlib(chunk_data) {
            Ok(decompressed) => result.extend_from_slice(&decompressed),
            Err(_) => {
                // Try raw deflate for this chunk
                let decompressed =
                    decompress_deflate(chunk_data).map_err(|e| e.to_string())?;
                result.extend_from_slice(&decompressed);
            }
        }
        chunks_found += 1;
    }

    if chunks_found == 0 {
        return Err("No valid chunks found in chunked ZLIB data".into());
    }

    Ok(result)
}

/// Compress data for ZTE config file restore.
pub fn compress(data: &[u8], chunked: bool, chunk_size: usize) -> Result<Vec<u8>, String> {
    if !chunked {
        return compress_zlib(data);
    }

    let mut result = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + chunk_size).min(data.len());
        let chunk = &data[offset..end];
        offset = end;
        let compressed = compress_zlib(chunk)?;
        result.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
        result.extend_from_slice(&compressed);
    }
    Ok(result)
}

fn compress_zlib(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}
