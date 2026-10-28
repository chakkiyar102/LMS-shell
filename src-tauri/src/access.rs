//! Gated-course access. An access code is "<expiry day>.<signature>": an Ed25519 signature over
//! "<brand::ACCESS_CODE_DOMAIN>|<username>|<expiry day>", minted offline by tools/access-code.mjs.
//! The app holds only the public key, so it can check codes but never make them.
//! offline check, so a code can't be revoked before it expires and course files in the install
//! aren't encrypted; a server-side check is the upgrade path if that matters.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::signature::{UnparsedPublicKey, ED25519};

/// A PLACEHOLDER: a fresh key whose private half was thrown away, so no code opens a gated course
/// until you run `node tools/access-code.mjs keygen` and paste your own public key here (docs/access.md).
const PUBLIC_KEY: [u8; 32] = [
    0x87, 0xdb, 0x8b, 0xd1, 0x5a, 0xb1, 0xd8, 0x66, 0x86, 0x03, 0xe3, 0x81, 0x6a, 0x65, 0x25, 0xa9,
    0x7c, 0x80, 0xe6, 0x63, 0x8b, 0x93, 0x39, 0xfd, 0x41, 0x99, 0x69, 0x4e, 0x84, 0x1e, 0x27, 0xfc,
];

/// The one shared login: PBKDF2-HMAC-SHA256 over "<username>\n<password>" (username lowercased,
/// both trimmed) with a random salt. Paste the output of `sh tools/hash-login.sh` here and release.
/// anyone holding it can pass it on and changing it needs a release; per-user codes are the controlled path.
const SHARED_ITERATIONS: u32 = 600_000;
const SHARED_SALT: [u8; 16] = [0; 16];
/// Made with tools/hash-login.sh. All zero (as shipped) means no shared login: checked before any hashing.
const SHARED_KEY: [u8; 32] = [0; 32];

const BAD: &str = "That username and access code don't match. Check both and try again.";

/// Days since 1970-01-01 (UTC), the unit expiry days are written in.
pub fn today() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0)
}

/// Checks a username and access code (or the shared login) and returns what to store on this
/// device for `check_stored`. The shared password itself is never stored: its marker is.
pub fn check(user: &str, code: &str, today: u64) -> Result<String, String> {
    match check_with(&PUBLIC_KEY, user, code, today) {
        Ok(()) => Ok(code.trim().to_string()),
        Err(_) if shared_login(&SHARED_SALT, &SHARED_KEY, SHARED_ITERATIONS, user, code) => Ok(shared_marker(&SHARED_KEY)),
        Err(err) => Err(err),
    }
}

/// Re-checks what `check` stored, at start-up: a signed code again (expiry included), or the shared
/// login's marker against the shipped key, so replacing the shared login relocks devices that used the old one.
pub fn check_stored(user: &str, stored: &str, today: u64) -> bool {
    if stored.starts_with(SHARED_PREFIX) {
        SHARED_KEY != [0; 32] && same(stored.as_bytes(), shared_marker(&SHARED_KEY).as_bytes())
    } else {
        check_with(&PUBLIC_KEY, user, stored, today).is_ok()
    }
}

const SHARED_PREFIX: &str = "shared:";

fn shared_marker(key: &[u8]) -> String { format!("{SHARED_PREFIX}{}", URL_SAFE_NO_PAD.encode(&key[..16])) }

/// Constant-time equality for equal-length inputs.
fn same(a: &[u8], b: &[u8]) -> bool { a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0 }

fn shared_login(salt: &[u8], key: &[u8; 32], iterations: u32, user: &str, password: &str) -> bool {
    if *key == [0; 32] { return false }
    let Some(n) = std::num::NonZeroU32::new(iterations) else { return false };
    let login = format!("{}\n{}", user.trim().to_lowercase(), password.trim());
    ring::pbkdf2::verify(ring::pbkdf2::PBKDF2_HMAC_SHA256, n, salt, login.as_bytes(), key).is_ok()
}

fn check_with(key: &[u8], user: &str, code: &str, today: u64) -> Result<(), String> {
    let user = user.trim().to_lowercase();
    let (exp, sig) = code.trim().split_once('.').ok_or(BAD)?;
    let exp: u64 = exp.parse().map_err(|_| BAD)?;
    let sig = URL_SAFE_NO_PAD.decode(sig).map_err(|_| BAD)?;
    UnparsedPublicKey::new(&ED25519, key)
        .verify(format!("{}|{user}|{exp}", shared::brand::ACCESS_CODE_DOMAIN).as_bytes(), &sig)
        .map_err(|_| BAD)?;
    if exp != 0 && today > exp { return Err("This access code has expired. Ask for a new one.".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check, check_with};

    use super::{check_stored, same, shared_login, shared_marker};

    // A throwaway login with its own salt and few iterations; the shipped one stays out of the repo.
    const T_SALT: [u8; 16] = *b"test-salt-000001";
    fn t_key(login: &str, iterations: u32) -> [u8; 32] {
        let mut out = [0u8; 32];
        ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA256, std::num::NonZeroU32::new(iterations).unwrap(), &T_SALT, login.as_bytes(), &mut out);
        out
    }

    #[test]
    fn shared_login_matches_only_its_username_and_password() {
        let key = t_key("casey.mercer\ncedar-harbor-17", 1_000);
        assert!(shared_login(&T_SALT, &key, 1_000, "casey.mercer", "cedar-harbor-17"));
        assert!(shared_login(&T_SALT, &key, 1_000, "  Casey.Mercer ", " cedar-harbor-17 ")); // trimmed, username case-insensitive
        assert!(!shared_login(&T_SALT, &key, 1_000, "casey.mercer", "Maple-lantern-42"), "password is case-sensitive");
        assert!(!shared_login(&T_SALT, &key, 1_000, "jordan.hale", "cedar-harbor-17"));
        assert!(!shared_login(&T_SALT, &key, 1_001, "casey.mercer", "cedar-harbor-17"), "iteration count is part of the hash");
        assert!(!shared_login(b"other-salt-00001", &key, 1_000, "casey.mercer", "cedar-harbor-17"), "salt is part of the hash");
        assert!(!shared_login(&T_SALT, &[0; 32], 1_000, "casey.mercer", "cedar-harbor-17"), "an all-zero key accepts nothing");
    }

    // tools/hash-login.sh hashes with Python's hashlib; this pins that both sides agree.
    #[test]
    fn hash_tool_and_ring_agree() {
        let hex: String = t_key("casey.mercer\ncedar-harbor-17", 1_000).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "9d1853a1a6a5cb3db1b51a3142b5ac14097900298d04aca26ec30a8b58a9aac5");
    }

    #[test]
    fn unconfigured_shared_login_accepts_nothing() {
        for (u, p) in [("", ""), ("casey.mercer", ""), ("casey.mercer", "cedar-harbor-17")] {
            assert!(check(u, p, 0).is_err());
        }
        // a forged marker can't stand in while no shared login ships
        assert!(!check_stored("casey.mercer", &shared_marker(&[0; 32]), 0));
        assert!(!check_stored("casey.mercer", "shared:", 0));
    }

    #[test]
    fn constant_time_compare() {
        assert!(same(b"abc", b"abc"));
        assert!(!same(b"abc", b"abd") && !same(b"abc", b"ab"));
    }

    // A throwaway test key, not the shipped one.
    const KEY: [u8; 32] = [
        0x03, 0x13, 0x27, 0xe9, 0x17, 0xb5, 0x61, 0x73, 0x74, 0xe8, 0xb5, 0x58, 0xfd, 0x1a, 0x44, 0x2b,
        0x57, 0xe6, 0x36, 0x0a, 0xc1, 0xf8, 0xfd, 0x38, 0xc3, 0x38, 0x2f, 0x55, 0x85, 0xab, 0x7b, 0x76,
    ];
    const NEVER: &str = "0.at-8f_EvvUiYZ4uYUOMHe0z7CS4KjPt_bjVtwi6XEJ3YtDtDGOVGD8QzhhoIR8BVXFdCTe1K2R1eDs3wTzqqDw";
    const EXP: &str = "20000.b46zMSxk_wRVY-8-cMi26hNQX2hYyzpDIk0L3aAXITihsjwAbdxbrl3myrmG8jeUC7qnNBjlgurTgyb8ClZ9Dg";

    #[test]
    fn valid_code_unlocks_for_its_user_only() {
        assert!(check_with(&KEY, "casey.mercer", NEVER, 99_999).is_ok());
        assert!(check_with(&KEY, "  Casey.Mercer ", NEVER, 0).is_ok()); // trimmed, case-insensitive
        assert!(check_with(&KEY, "jordan.hale", NEVER, 0).is_err());
    }

    #[test]
    fn expiry_is_enforced() {
        assert!(check_with(&KEY, "casey.mercer", EXP, 20_000).is_ok());
        assert!(check_with(&KEY, "casey.mercer", EXP, 20_001).is_err());
        // editing the expiry breaks the signature
        assert!(check_with(&KEY, "casey.mercer", &EXP.replacen("20000", "0", 1), 0).is_err());
    }

    #[test]
    fn garbage_is_rejected() {
        for code in ["", ".", "0.", "x.abc", "0.!!!", NEVER.trim_end_matches('w')] {
            assert!(check_with(&KEY, "casey.mercer", code, 0).is_err(), "{code}");
        }
    }

    #[test]
    fn the_shipped_constants_are_placeholders() {
        assert_eq!(super::SHARED_KEY, [0; 32], "the shared login ships disabled");
        assert!(check("casey.mercer", NEVER, 0).is_err(), "a test code must not open the shipped key");
    }
}
