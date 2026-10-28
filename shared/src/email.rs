//! Email domain rules for the profile's work email, shared by the UI and the backend so both
//! give the same answer: an allowlist first, then a short table of typos of the big webmail
//! providers, then the disposable-provider blocklist (the domain and every parent domain). Free
//! webmail itself is accepted. Whether the domain exists in DNS is checked by the backend only.
use std::sync::OnceLock;

pub const DISPOSABLE_ERROR: &str = "Use your work email. Temporary or disposable addresses aren't accepted.";
pub const NON_ASCII_ERROR: &str = "Use the plain-letter form of your email domain. Domains with accented or non-Latin letters aren't supported.";

const DISPOSABLE: &str = include_str!("../data/disposable_domains.txt");
const ALLOWED: &str = include_str!("../data/allowed_domains.txt");

/// Exact typos of the big providers and the domain meant. Only exact matches, so real domains
/// that merely look similar are never touched.
const TYPOS: &[(&str, &str)] = &[
    ("gmial.com", "gmail.com"), ("gmai.com", "gmail.com"), ("gmal.com", "gmail.com"), ("gamil.com", "gmail.com"),
    ("gnail.com", "gmail.com"), ("gmaill.com", "gmail.com"), ("gmail.con", "gmail.com"), ("gmail.cm", "gmail.com"),
    ("gmail.cmo", "gmail.com"),
    ("hotmial.com", "hotmail.com"), ("hotmal.com", "hotmail.com"), ("hotmai.com", "hotmail.com"), ("hotamil.com", "hotmail.com"),
    ("hotmail.con", "hotmail.com"),
    ("yaho.com", "yahoo.com"), ("yahooo.com", "yahoo.com"), ("yhoo.com", "yahoo.com"), ("yahoo.con", "yahoo.com"),
    ("outlok.com", "outlook.com"), ("outllok.com", "outlook.com"), ("outloo.com", "outlook.com"), ("outlook.con", "outlook.com"),
    ("iclod.com", "icloud.com"), ("icoud.com", "icloud.com"),
];

fn entries(text: &'static str) -> Vec<&'static str> {
    let mut v: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    v.sort_unstable();
    v.dedup();
    v
}

fn disposable() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| entries(DISPOSABLE))
}

fn allowed() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| entries(ALLOWED))
}

/// The domain and each parent with at least two labels: a.b.mailinator.com, b.mailinator.com, mailinator.com.
fn self_and_parents(domain: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(domain), |d| d.split_once('.').map(|(_, rest)| rest)).filter(|d| d.contains('.'))
}

/// Lowercase, trimmed, without a trailing dot.
pub fn normalise_domain(domain: &str) -> String {
    domain.trim().trim_end_matches('.').to_lowercase()
}

/// Whether the domain (or any parent of it) is on the allowlist.
pub fn is_allowed(domain: &str) -> bool {
    self_and_parents(domain).any(|d| allowed().binary_search(&d).is_ok())
}

/// Whether the domain (or any parent of it) is a known disposable-email provider.
pub fn is_disposable(domain: &str) -> bool {
    self_and_parents(domain).any(|d| disposable().binary_search(&d).is_ok())
}

/// The provider domain meant by a known typo, e.g. gmial.com gives gmail.com.
pub fn typo_fix(domain: &str) -> Option<&'static str> {
    TYPOS.iter().find(|(typo, _)| *typo == domain).map(|(_, fix)| *fix)
}

/// The domain rules for a normalised (lowercase, no trailing dot) address `local@domain`.
pub fn check_domain(local: &str, domain: &str) -> Result<(), String> {
    if !domain.is_ascii() { return Err(NON_ASCII_ERROR.into()) }
    if is_allowed(domain) { return Ok(()) }
    // typos first: a few (gmial.com, hotmial.com) are also on the disposable list, and the fix is the better answer
    if let Some(fix) = typo_fix(domain) { return Err(format!("Did you mean {local}@{fix}?")) }
    if is_disposable(domain) { return Err(DISPOSABLE_ERROR.into()) }
    Ok(())
}

/// Embedded list size, for the size note in docs and tests.
pub fn disposable_count() -> usize { disposable().len() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_is_loaded_and_has_no_webmail() {
        assert!(disposable_count() > 5000, "{}", disposable_count());
        for d in ["gmail.com", "outlook.com", "hotmail.com", "yahoo.com", "icloud.com", "brightline.example"] {
            assert!(!is_disposable(d), "{d}");
        }
        for (_, fix) in TYPOS { assert!(!is_disposable(fix), "{fix}"); }
    }

    #[test]
    fn parents_are_matched() {
        assert!(is_disposable("mailinator.com") && is_disposable("sub.mailinator.com") && is_disposable("a.b.mailinator.com"));
        assert!(!is_disposable("com") && !is_disposable("northwind-labs.example") && !is_disposable("mailinator.com.northwind-labs.example"));
        assert!(is_allowed("brightline.example") && is_allowed("mail.brightline.example") && !is_allowed("notbrightline.example"));
    }

    #[test]
    fn normalises() {
        assert_eq!(normalise_domain(" MailInator.COM. "), "mailinator.com");
    }
}
