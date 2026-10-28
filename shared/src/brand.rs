//! Everything a fork changes to rebrand the app, in one place (see "Rebrand in 5 steps" in README.md).
//! The UI and the backend both read these, so one edit here renames the app everywhere.
//!
//! The two names are macros as well as consts so they can be glued into `&'static str` text at
//! compile time with `concat!`: `concat!("Ask ", shared::companion_name!())`.

/// The app's display name: window title, top bar, certificate, onboarding.
#[macro_export]
macro_rules! app_name { () => { "LMS Shell" } }

/// The companion's display name: its tab, its pane, its chat and every line that names it.
#[macro_export]
macro_rules! companion_name { () => { "Guide" } }

pub const APP_NAME: &str = app_name!();
pub const COMPANION: &str = companion_name!();

/// An optional word shown after the logo in the top bar (for example `Some("Academy")`). `None` shows
/// the logo alone.
pub const BRAND_SUFFIX: Option<&str> = None;

/// The short tagline under the app name on the onboarding screen.
pub const TAGLINE: &str = "Learn at your own pace";

/// The roles learners pick from on the onboarding screen, as (id, label, hint). The ids are fixed by
/// [`crate::Role`]; change the labels and hints freely. A role that `asks_code` offers the access-code
/// form right after it is picked (with "I'll add it later" to skip).
pub const ROLES: [(crate::Role, &str, &str); 3] = [
    (crate::Role::Learner, "Learner", "Every open course is yours. Gated courses open with an access code."),
    (crate::Role::Partner, "Partner", "Gated courses open with the username and access code from your contact."),
    (crate::Role::Staff, "Staff", "Gated courses open with an access code from your team."),
];

/// Roles that are offered the access-code form during onboarding.
pub const ROLES_ASKING_CODE: [crate::Role; 2] = [crate::Role::Partner, crate::Role::Staff];

/// When set, the Staff role must use an email on this domain (or a subdomain), e.g. `Some("example.org")`.
/// `None` accepts any valid work email for every role.
pub const STAFF_EMAIL_DOMAIN: Option<&str> = None;

/// Refuse disposable and mistyped webmail domains at onboarding (`shared/data/disposable_domains.txt`).
pub const CHECK_DISPOSABLE_EMAIL: bool = true;

/// Also ask DNS whether the email domain exists (MX, then A/AAAA). Offline or slow lookups always allow.
pub const CHECK_EMAIL_DNS: bool = true;

/// The control domain the DNS check resolves to tell "no such domain" from "no network".
pub const DNS_CONTROL_DOMAIN: &str = "example.com";

/// What learners practise in, as it reads in a sentence: "Open your practice app beside the lesson".
pub const PRACTICE_NOUN: &str = "practice app";

/// The practice pane only opens https URLs on this host or its subdomains, e.g. `Some("example.com")`.
/// `None` (the default) accepts any https URL the learner enters.
pub const PRACTICE_HOST_SUFFIX: Option<&str> = None;

/// An example practice address for placeholders and messages.
pub const PRACTICE_EXAMPLE: &str = "app.example.com";

/// Lesson "Read more" links must start with this prefix (validated in `help_url_ok` and tools/validate.py).
/// `"https://"` accepts any https link; set it to your docs site, e.g. `"https://docs.example.com/"`.
pub const HELP_URL_PREFIX: &str = "https://";

/// What the "Read more" links on a lesson are called.
pub const HELP_SITE_NAME: &str = "the docs";

/// The access-code tool's signing domain: part of the signed text, so codes from one fork never open another.
/// Change it when you generate your own keypair (tools/access-code.mjs reads the same value).
pub const ACCESS_CODE_DOMAIN: &str = "lms-shell-access";

/// Self-updates. Off by default: turning it on also needs `plugins.updater` (your public key and an
/// endpoint) in src-tauri/tauri.conf.json and signed release artifacts. See docs/updates.md.
pub const UPDATER_ENABLED: bool = false;
