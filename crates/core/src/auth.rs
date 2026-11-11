use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use hmac::{Hmac, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

pub const TOTP_STEP: u64 = 30;
pub const TOTP_DIGITS: u32 = 6;

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::encode_b64(&random_bytes(16)?).map_err(|error| error.to_string())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| error.to_string())
}

pub fn verify_password(password: &str, encoded: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(encoded) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn decode_base32(secret: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = secret
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '=')
        .collect();
    data_encoding::BASE32
        .decode(cleaned.to_uppercase().as_bytes())
        .map_err(|_| "segredo TOTP inválido".to_string())
}

pub fn totp_code_at(secret: &str, unix_time: u64) -> Result<String, String> {
    let key = decode_base32(secret)?;
    let counter = unix_time / TOTP_STEP;
    let mut mac = HmacSha1::new_from_slice(&key).map_err(|error| error.to_string())?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 0x0f);
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    let code = binary % 10u32.pow(TOTP_DIGITS);
    Ok(format!("{code:0width$}", width = TOTP_DIGITS as usize))
}

pub fn verify_totp(secret: &str, code: &str, unix_time: u64) -> bool {
    let code = code.trim();
    if code.len() != TOTP_DIGITS as usize || !code.chars().all(|digit| digit.is_ascii_digit()) {
        return false;
    }
    for offset in [-1i64, 0, 1] {
        let seconds = unix_time as i64 + offset * TOTP_STEP as i64;
        if seconds < 0 {
            continue;
        }
        if let Ok(expected) = totp_code_at(secret, seconds as u64) {
            if constant_time_eq(expected.as_bytes(), code.as_bytes()) {
                return true;
            }
        }
    }
    false
}

pub fn random_secret() -> Result<String, String> {
    Ok(data_encoding::BASE32_NOPAD.encode(&random_bytes(20)?))
}

fn random_bytes(length: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;

    let mut bytes = vec![0u8; length];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

pub fn otpauth_uri(issuer: &str, account: &str, secret: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={secret}&issuer={}&digits={TOTP_DIGITS}&period={TOTP_STEP}",
        percent_encode(issuer),
        percent_encode(account),
        percent_encode(issuer),
    )
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn totp_matches_rfc6238_vector() {
        assert_eq!(totp_code_at(RFC_SECRET, 59).unwrap(), "287082");
        assert_eq!(totp_code_at(RFC_SECRET, 1_111_111_109).unwrap(), "081804");
    }

    #[test]
    fn totp_accepts_previous_and_next_step() {
        let now = 1_700_000_000u64;
        let code = totp_code_at(RFC_SECRET, now).unwrap();
        assert!(verify_totp(RFC_SECRET, &code, now));
        assert!(verify_totp(RFC_SECRET, &code, now + TOTP_STEP));
        assert!(verify_totp(RFC_SECRET, &code, now - TOTP_STEP));
        assert!(!verify_totp(RFC_SECRET, &code, now + 10 * TOTP_STEP));
        assert!(!verify_totp(RFC_SECRET, "abc", now));
    }

    #[test]
    fn password_hash_round_trip() {
        let encoded = hash_password("senha-secreta").unwrap();
        assert!(encoded.starts_with("$argon2"));
        assert!(verify_password("senha-secreta", &encoded));
        assert!(!verify_password("outra", &encoded));
        assert!(!verify_password("senha-secreta", "inválido"));
    }

    #[test]
    fn random_secret_is_decodable_base32() {
        let secret = random_secret().unwrap();
        assert_eq!(secret.len(), 32);
        assert_eq!(decode_base32(&secret).unwrap().len(), 20);
        assert!(decode_base32("não-base32!").is_err());
    }

    #[test]
    fn otpauth_uri_encodes_issuer() {
        let uri = otpauth_uri("Like Terminal", "felipe@example.com", "ABC123");
        assert!(
            uri.starts_with("otpauth://totp/Like%20Terminal:felipe%40example.com?secret=ABC123")
        );
        assert!(uri.contains("issuer=Like%20Terminal"));
        assert!(uri.contains("digits=6"));
        assert!(uri.contains("period=30"));
    }
}
