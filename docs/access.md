# Access

Who can open which course, how per-person access codes work, and how to set up the optional shared login. The code is in `src-tauri/src/access.rs` (checks), `src-tauri/src/lib.rs` (gating) and `shared/src/brand.rs` plus `shared/src/lib.rs` (roles and profile rules).

## Roles

Onboarding asks for a name, a work email and a role. The roles are `Learner`, `Partner` and `Staff` out of the box; their labels and the hint under the select are in `ROLES` in `shared/src/brand.rs`. Roles in `ROLES_ASKING_CODE` (Partner and Staff by default) are offered the access-code fields right after they pick, with **I'll add it later** to skip.

The role and email are typed, not verified, so **no role opens a gated course by itself**. Everyone, Staff included, needs a code or the shared login.

- Set `STAFF_EMAIL_DOMAIN` to `Some("your-org.example")` to require Staff to use that domain (or a subdomain). `None` accepts any valid work email.
- To add or remove a role, change the `Role` enum in `shared/src/lib.rs` and `ROLES` in `brand.rs`; the stored value is the lowercase variant name.

## Gated courses

A course whose `course.yaml` sets `gated: true` is locked unless the device holds a valid access code or the shared login's marker.

- A locked course shows a lock in the top menu and, when opened, an unlock form (username and access code).
- Every content command refuses a locked course, so lessons, checks and the certificate stay closed.
- The lock is re-evaluated at start-up (an expired code relocks the course), after onboarding, after unlocking and after removing the code. Progress is never touched.
- Learners can enter a code during onboarding, on the locked course, or in **Settings, Access**, and remove it there too.

## Per-person access codes

A code is tied to one username and has an expiry:

```
<expiry>.<signature>
```

- `<expiry>` is a day number, days since 1970-01-01 in UTC. `0` means never. The expiry day is inclusive.
- `<signature>` is an Ed25519 signature, base64url without padding, over `<ACCESS_CODE_DOMAIN>|<username>|<expiry>`, where `ACCESS_CODE_DOMAIN` is in `shared/src/brand.rs`. Changing it means codes from another fork never open yours.
- The username is trimmed and lowercased before signing and checking. Pick one convention (for example `first.last`) and send it exactly as the tool prints it.

The app ships only the 32-byte public key, `PUBLIC_KEY` in `src-tauri/src/access.rs`. It can check codes but never make them.

> **The shipped key is a placeholder.** It was generated for this repository and its private half was thrown away, so no code can open a gated course until you put in your own.

### Make your keypair (once)

```bash
node tools/access-code.mjs keygen
```

It writes the private key to `~/.config/lms-shell/access-signing.pem` (or the path in `LMS_ACCESS_KEY`), refuses to overwrite an existing one, and prints a `PUBLIC_KEY` block. Paste it over the one in `src-tauri/src/access.rs`, run `cargo test`, and release.

Back up the private key. Every code you issue depends on it; losing it means a new public key, a release, and new codes for everyone. Never commit it.

### Issue a code

```bash
node tools/access-code.mjs casey.mercer --days 90
```

It prints the username, the code and the last valid day (`--days` defaults to 365; `--days 0` never expires). Send both to that person privately. Day boundaries are midnight UTC.

**Limits.** A code can't be revoked before it expires, because the check is offline. Course files inside the install are not encrypted. A server-side check is the upgrade path if either matters to you.

## The shared login (optional)

One username and password for everyone. Only a hash ships: PBKDF2-HMAC-SHA256 with 600,000 iterations and a random 16-byte salt, over `<username>\n<password>` (username lowercased, both trimmed). The constants are `SHARED_SALT` and `SHARED_KEY` in `src-tauri/src/access.rs`.

**It ships all zero, which means off:** no shared login is accepted.

To turn it on or change it:

1. Run `sh tools/hash-login.sh`. It asks for the username and the password twice without echoing them and prints new `SHARED_SALT` and `SHARED_KEY` constants. Nothing is written to disk.
2. Paste them over the existing ones in `access.rs`.
3. Run `cargo test -p lms-shell` and release.
4. Hand the credentials to learners outside the repository.

A device that unlocks with the shared login stores a marker derived from `SHARED_KEY`, never the password. Replacing the constants relocks those devices once they run the new build; installs that have not updated keep accepting the old login. Anyone holding a shared login can pass it on, so prefer per-person codes.

## Removing a code and starting over

- **Settings, Access, Remove access code** deletes the stored username and code. Gated courses lock again on this device; the profile and progress stay.
- **Settings, Start over on this computer** erases the learner database (profile, progress, attempts, chats, certificates, activity, settings, access code) and the saved AI keys, then reloads into onboarding. The course content, the practice app's cookies and anything outside the app's data folder and keychain entries are kept.

## Tests

`cargo test -p lms-shell` covers codes for the right user only, expiry, garbage input, the shared login (case rules, salt and iteration count), the all-zero key, the marker, the gating matrix, removing the code and Start over. The tests use throwaway keys, never the shipped placeholder.
