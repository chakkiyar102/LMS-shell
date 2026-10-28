#!/bin/sh
# Prints the Rust constants for the shared login in src-tauri/src/access.rs:
# PBKDF2-HMAC-SHA256, 600,000 iterations, a fresh random 16-byte salt, over "<username>\n<password>"
# (username lowercased, both trimmed, as the app checks them).
# The password is read without echo and passed to python on stdin, never on a command line;
# nothing is written to disk or logged. Paste the output over SHARED_SALT and SHARED_KEY, then release.
set -eu
printf 'Shared login username: ' >&2
read -r user
if [ -t 0 ]; then
  stty_saved=$(stty -g)
  trap 'stty "$stty_saved"' EXIT INT TERM
  stty -echo
fi
printf 'Password: ' >&2
read -r pass
printf '\nPassword again: ' >&2
read -r again
[ -t 0 ] && stty "$stty_saved"
printf '\n' >&2
[ "$pass" = "$again" ] || { echo "The passwords don't match." >&2; exit 1; }
[ -n "$user" ] && [ -n "$pass" ] || { echo "Username and password are both needed." >&2; exit 1; }
printf '%s\n%s' "$user" "$pass" | /usr/bin/env python3 -I -c '
import hashlib, os, sys
user, _, pw = sys.stdin.read().partition("\n")
salt = os.urandom(16)
key = hashlib.pbkdf2_hmac("sha256", (user.strip().lower() + "\n" + pw.strip()).encode(), salt, 600_000)
rows = lambda b: "\n".join("    " + " ".join(f"0x{x:02x}," for x in b[i:i + 16]) for i in range(0, len(b), 16))
print(f"const SHARED_SALT: [u8; 16] = [\n{rows(salt)}\n];")
print(f"const SHARED_KEY: [u8; 32] = [\n{rows(key)}\n];")
'
