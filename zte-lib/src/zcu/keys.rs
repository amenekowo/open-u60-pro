use md5::{Digest, Md5};

/// Known static AES keys for ZTE router config files.
/// Each entry: (description, key_bytes).
pub const KNOWN_KEYS: &[(&str, &[u8])] = &[
    ("ZTE default (MIIBIjANB...)", b"MIIBIjANBgkqhk"),
    ("ZTE default 2", b"Wj"),
    ("ZTE ZXHN H298N", b"ZTE%FN$GponNJ025"),
    ("ZTE ZXHN H108N V2.5", b"GrWM2ans*f@7SSc&"),
    ("ZTE ZXHN H168N V3.5", b"GrWM3mn/\0Y>*f2gU"),
    ("ZTE ZXHN H298A", b"m8@96&ah*ZTE%FN!"),
    ("ZTE ZXHN F670L", b"ZTE%FN$GponNJ025"),
    ("ZTE MF283+", b"SDT&*Ssym0722!@#"),
    ("ZTE ZXHN F609", b"'MMI@FP*Jhg&^%$$"),
    ("ZTE ZXHN F660", b"ZTE%FN$GponNJ025"),
    ("ZTE ZXHN H267A", b"GrWM2ans*f@7SSc&"),
    ("ZTE generic key 1", b"402c38de39bed665"),
    ("ZTE generic key 2", b"8cc72b05705d5c46"),
    ("ZTE generic key 3", b"SMGPOINTzteGpon!"),
];

/// Derive a 16-byte AES key from a device serial number via MD5.
pub fn key_from_serial(serial: &str) -> Vec<u8> {
    let mut hasher = Md5::new();
    hasher.update(serial.as_bytes());
    hasher.finalize()[..16].to_vec()
}

/// Derive a 16-byte AES key from the config file signature field via MD5.
pub fn key_from_sig(signature: &[u8]) -> Vec<u8> {
    let mut hasher = Md5::new();
    hasher.update(signature);
    hasher.finalize()[..16].to_vec()
}

/// Return all candidate keys to try, including derived ones.
pub fn get_all_keys(
    serial: Option<&str>,
    signature: Option<&[u8]>,
) -> Vec<(String, Vec<u8>)> {
    let mut candidates: Vec<(String, Vec<u8>)> = Vec::new();

    if let Some(sig) = signature {
        if !sig.is_empty() {
            candidates.push(("derived from signature".to_string(), key_from_sig(sig)));
        }
    }
    if let Some(ser) = serial {
        if !ser.is_empty() {
            candidates.push((
                format!("derived from serial {ser}"),
                key_from_serial(ser),
            ));
        }
    }

    for &(desc, key) in KNOWN_KEYS {
        candidates.push((desc.to_string(), key.to_vec()));
    }

    candidates
}
