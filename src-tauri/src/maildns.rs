//! Does the profile's email domain exist? Backend only (the UI can't resolve names).
//!
//! MX first (a mail-only domain with no address record is a real, valid work domain), then A/AAAA,
//! through the system's DNS servers with a short timeout. Only "no records" counts as missing;
//! any other resolver error is Unknown. The system resolver reports "no such host" the same way
//! when the machine is offline, so a failed lookup is confirmed against a control domain: if that
//! one fails too, the learner is offline and the address is allowed. A timeout also allows it.
use std::net::ToSocketAddrs;
use std::sync::mpsc;
use std::time::Duration;

pub const NOT_FOUND_ERROR: &str = "We couldn't find that email domain. Check for typos.";
const TIMEOUT: Duration = Duration::from_secs(3);
/// Resolves whenever DNS works at all; used only to tell "no such domain" from "offline".
const CONTROL: &str = shared::brand::DNS_CONTROL_DOMAIN;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lookup {
    /// The name has at least one address.
    Found,
    /// The resolver answered with an error or no addresses.
    NotFound,
    /// No answer in time.
    Unknown,
}

/// How a name is looked up; the real one is `system_lookup`, tests pass a stub.
pub type Resolver = fn(&str) -> Lookup;

/// MX, then A/AAAA, abandoned after `TIMEOUT` (the lookup thread then finishes on its own and its
/// answer is dropped).
pub fn system_lookup(host: &str) -> Lookup {
    let host = host.to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mx = mx_lookup(&host);
        let found = mx == Lookup::Found
            || (host.as_str(), 0).to_socket_addrs().map(|mut a| a.next().is_some()).unwrap_or(false);
        // an MX resolver error (not "no records") means DNS couldn't say; don't call the domain missing
        let _ = tx.send(if found { Lookup::Found } else if mx == Lookup::Unknown { Lookup::Unknown } else { Lookup::NotFound });
    });
    rx.recv_timeout(TIMEOUT).unwrap_or(Lookup::Unknown)
}

/// MX records through the system's DNS config. Found when any exist, NotFound only when DNS
/// answered "no records" (incl. no such domain), Unknown for every other failure.
fn mx_lookup(host: &str) -> Lookup {
    use hickory_resolver::error::ResolveErrorKind;
    let Ok(r) = hickory_resolver::Resolver::from_system_conf() else { return Lookup::Unknown };
    match r.mx_lookup(format!("{host}.")) {
        Ok(l) if l.iter().next().is_some() => Lookup::Found,
        Ok(_) => Lookup::NotFound,
        Err(e) if matches!(e.kind(), ResolveErrorKind::NoRecordsFound { .. }) => Lookup::NotFound,
        Err(_) => Lookup::Unknown,
    }
}

/// The error for a normalised domain that DNS says doesn't exist; None when it exists, when it is
/// on the allowlist, or when the lookup can't tell (offline, timed out).
pub fn domain_error(domain: &str, resolve: Resolver) -> Option<String> {
    if shared::email::is_allowed(domain) { return None }
    match resolve(domain) {
        Lookup::Found | Lookup::Unknown => None,
        Lookup::NotFound => (resolve(CONTROL) == Lookup::Found).then(|| NOT_FOUND_ERROR.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{domain_error, Lookup, NOT_FOUND_ERROR};

    #[test]
    fn offline_or_slow_lookups_allow_the_address() {
        // offline: every lookup fails, the control domain too
        assert_eq!(domain_error("northwind-labs.example", |_| Lookup::NotFound), None);
        // timed out
        assert_eq!(domain_error("northwind-labs.example", |_| Lookup::Unknown), None);
        // control resolves but times out on the domain itself
        assert_eq!(domain_error("northwind-labs.example", |h| if h == super::CONTROL { Lookup::Found } else { Lookup::Unknown }), None);
    }

    #[test]
    fn a_missing_domain_is_refused_when_dns_works() {
        let online = |h: &str| if h == super::CONTROL || h == "northwind-labs.example" { Lookup::Found } else { Lookup::NotFound };
        assert_eq!(domain_error("northwind-labs.example", online), None);
        assert_eq!(domain_error("northwind-labss.example", online).as_deref(), Some(NOT_FOUND_ERROR));
    }

    /// Uses the network; run on request: cargo test --lib maildns -- --ignored
    #[test]
    #[ignore]
    fn real_dns_mail_only_and_missing_domains() {
        assert_eq!(super::system_lookup("example.com"), Lookup::Found);
        assert_eq!(super::system_lookup("no-such-domain-lms-shell-test-0x7f.com"), Lookup::NotFound);
    }

    #[test]
    fn allowlisted_domains_skip_the_lookup() {
        assert_eq!(domain_error("brightline.example", |_| panic!("no lookup for an allowlisted domain")), None);
    }
}
