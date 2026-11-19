//! Per-device setup token for management writes from the home network.
//!
//! The token is eight characters of a 32-symbol alphabet (40 bits), generated
//! once per device and stored in NVS. It is never logged and never written to
//! `state.json`; the only way to read it is `GET /api/firmware` from inside the
//! station subnet. The token is not an authentication system: it keeps casual
//! LAN neighbours and cross-site requests out, while anyone who can sniff the
//! plaintext HTTP session can read it.
use crate::nvs_store::FirmwareNvs;

pub const SETUP_TOKEN_LEN: usize = 8;
pub const SETUP_TOKEN_KEY: &str = "setup_token";
pub const SETUP_TOKEN_HEADER: &str = "X-Matrix-Setup-Token";
/// 32 symbols without `I`, `O`, `0` and `1` to survive being read aloud.
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn encode(bytes: [u8; 5]) -> String {
    let mut acc: u64 = 0;
    for byte in bytes {
        acc = (acc << 8) | u64::from(byte);
    }
    let mut out = String::with_capacity(SETUP_TOKEN_LEN);
    for index in 0..SETUP_TOKEN_LEN {
        let shift = 35 - index * 5;
        out.push(ALPHABET[((acc >> shift) & 0x1f) as usize] as char);
    }
    out
}

pub fn is_well_formed(token: &str) -> bool {
    token.len() == SETUP_TOKEN_LEN
        && token
            .bytes()
            .all(|byte| ALPHABET.contains(&byte.to_ascii_uppercase()))
}

#[cfg(target_os = "espidf")]
fn random_bytes() -> [u8; 5] {
    let mut bytes = [0u8; 5];
    unsafe { esp_idf_sys::esp_fill_random(bytes.as_mut_ptr() as *mut _, bytes.len()) };
    bytes
}

/// Host fallback for tests and the simulator only; ESP targets always use the
/// hardware RNG.
#[cfg(not(target_os = "espidf"))]
fn random_bytes() -> [u8; 5] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos() as u64)
        .unwrap_or(0);
    let stack = &nanos as *const u64 as u64;
    let mut state = nanos ^ stack.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15;
    let mut bytes = [0u8; 5];
    for slot in bytes.iter_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *slot = (state >> 24) as u8;
    }
    bytes
}

pub fn generate() -> String {
    encode(random_bytes())
}

/// Read the device token, provisioning a fresh one on first boot. A malformed
/// stored value is replaced. The value is never logged.
pub fn load_or_create(nvs: &FirmwareNvs) -> Result<String, String> {
    if let Some(existing) = nvs.get_str(SETUP_TOKEN_KEY)? {
        if is_well_formed(&existing) {
            return Ok(existing);
        }
        log::warn!("Stored setup token was malformed; provisioning a new one");
    }
    let token = generate();
    nvs.set_str(SETUP_TOKEN_KEY, &token)?;
    log::info!("Provisioned a per-device setup token ({} characters)", SETUP_TOKEN_LEN);
    Ok(token)
}

/// Constant-time-ish comparison, case-insensitive for typed input.
pub fn verify(expected: &str, provided: Option<&str>) -> bool {
    let Some(provided) = provided else {
        return false;
    };
    let provided = provided.trim();
    if !is_well_formed(expected) || provided.len() != SETUP_TOKEN_LEN {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in expected.bytes().zip(provided.bytes()) {
        difference |= a.to_ascii_uppercase() ^ b.to_ascii_uppercase();
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_tokens_are_eight_symbols_from_the_safe_alphabet() {
        let token = encode([0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(token.len(), SETUP_TOKEN_LEN);
        assert!(is_well_formed(&token));
        assert_eq!(encode([0xff, 0xff, 0xff, 0xff, 0xff]), "99999999");
        assert!(!is_well_formed("SHORT"));
        assert!(!is_well_formed("ABCDEFG0"));
    }

    #[test]
    fn verification_rejects_missing_short_and_wrong_tokens() {
        let token = "ABCDEFGH";
        assert!(verify(token, Some(token)));
        assert!(verify(token, Some("abcdefgh")));
        assert!(verify(token, Some(" abcdefgh ")));
        assert!(!verify(token, None));
        assert!(!verify(token, Some("")));
        assert!(!verify(token, Some("ABCDEFG")));
        assert!(!verify(token, Some("ABCDEFGJ")));
        assert!(!verify("", Some("")));
        assert!(!verify("SHORT", Some("SHORT")));
    }

    #[test]
    fn provisioning_is_stable_across_reloads() {
        let nvs = FirmwareNvs::new().unwrap();
        let first = load_or_create(&nvs).unwrap();
        let second = load_or_create(&nvs).unwrap();
        assert_eq!(first, second);
        assert!(is_well_formed(&first));
        assert_eq!(nvs.get_str(SETUP_TOKEN_KEY).unwrap().as_deref(), Some(first.as_str()));
    }

    #[test]
    fn malformed_stored_token_is_replaced() {
        let nvs = FirmwareNvs::new().unwrap();
        nvs.set_str(SETUP_TOKEN_KEY, "bad").unwrap();
        let token = load_or_create(&nvs).unwrap();
        assert!(is_well_formed(&token));
        assert_ne!(token, "bad");
    }
}
