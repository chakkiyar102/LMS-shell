//! The learner's practice-app address, as typed at onboarding or in Settings. The backend's
//! practice::validate_url uses this same rule when it is saved, so the form can say what's wrong
//! while the learner types: a bare host gets https://, only https is accepted, and when the fork
//! sets `brand::PRACTICE_HOST_SUFFIX` the host must be that domain or a subdomain of it.
use crate::brand;

/// True when `host` is `suffix` or a subdomain of it (case-insensitive, trailing dots ignored).
pub fn under_suffix(host: &str, suffix: &str) -> bool {
    let (h, s) = (host.trim_end_matches('.').to_ascii_lowercase(), suffix.trim_end_matches('.').trim_start_matches('.').to_ascii_lowercase());
    !s.is_empty() && (h == s || h.ends_with(&format!(".{s}")))
}

/// The address as it will be saved (https://host/...), or why it can't be used.
pub fn tenant_url(raw: &str) -> Result<String, String> { tenant_url_with(raw, brand::PRACTICE_HOST_SUFFIX) }

pub fn tenant_url_with(raw: &str, suffix: Option<&str>) -> Result<String, String> {
    let bad = || format!("Enter your {}'s address, like {}.", brand::PRACTICE_NOUN, suffix.map(|s| format!("app.{s}")).unwrap_or(brand::PRACTICE_EXAMPLE.into()));
    let raw = raw.trim();
    if raw.is_empty() || raw.chars().any(char::is_whitespace) { return Err(bad()); }
    let (scheme, rest) = match raw.split_once("://") { Some((s, r)) => (s.to_ascii_lowercase(), r), None => ("https".into(), raw) };
    if scheme != "https" { return Err(format!("Use the https:// address of your {}.", brand::PRACTICE_NOUN)); }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') { return Err(bad()); }
    let (host, port) = match authority.rsplit_once(':') { Some((h, p)) => (h, Some(p)), None => (authority, None) };
    if port.is_some_and(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) { return Err(bad()); }
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let labels_ok = host.contains('.') && host.split('.').all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    if !labels_ok { return Err(bad()); }
    if let Some(s) = suffix {
        if !under_suffix(&host, s) { return Err(format!("That address isn't on {s}. Use one that ends in .{s}.")); }
    }
    let path = &rest[authority.len()..];
    Ok(format!("https://{host}{}{}", port.map(|p| format!(":{p}")).unwrap_or_default(), if path.is_empty() { "/" } else { path }))
}

#[cfg(test)]
mod tests {
    use super::{tenant_url_with, under_suffix};

    #[test]
    fn bare_hosts_get_https_and_only_https_passes() {
        assert_eq!(tenant_url_with(" notes.brightline.example ", None).unwrap(), "https://notes.brightline.example/");
        assert_eq!(tenant_url_with("https://Notes.Brightline.Example/view", None).unwrap(), "https://notes.brightline.example/view");
        assert_eq!(tenant_url_with("notes.brightline.example:8443/a", None).unwrap(), "https://notes.brightline.example:8443/a");
        assert!(tenant_url_with("http://notes.brightline.example", None).is_err());
        assert!(tenant_url_with("https://notes.brightline.example@evil.example", None).is_err());
        assert!(tenant_url_with("notes .brightline.example", None).is_err());
        assert!(tenant_url_with("localhost", None).is_err());
        assert!(tenant_url_with("", None).is_err());
        assert!(tenant_url_with("javascript:alert(1)", None).is_err());
    }

    #[test]
    fn a_suffix_limits_the_hosts() {
        let s = Some("brightline.example");
        assert_eq!(tenant_url_with("notes.brightline.example", s).unwrap(), "https://notes.brightline.example/");
        assert!(tenant_url_with("brightline.example", s).is_ok());
        for bad in ["example.com", "brightline.example.evil.com", "evilbrightline.example"] {
            assert!(tenant_url_with(bad, s).is_err(), "{bad}");
        }
        assert!(under_suffix("A.Brightline.Example.", "brightline.example"));
        assert!(!under_suffix("brightline.example", ""));
    }
}
