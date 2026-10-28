#!/usr/bin/env node
// Access codes for gated courses. Each code is "<expiry day>.<signature>": an Ed25519 signature over
// "<domain>|<username>|<expiry day>", where <domain> is ACCESS_CODE_DOMAIN in shared/src/brand.rs.
// The private key never leaves this machine; the app ships only the public key (src-tauri/src/access.rs).
//   node tools/access-code.mjs keygen                 create the key (once) and print the public key for access.rs
//   node tools/access-code.mjs <username> [--days N]  print an access code (N days valid; default 365, 0 = never expires)
// The key lives at ~/.config/lms-shell/access-signing.pem, or the path in LMS_ACCESS_KEY.
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const KEY = process.env.LMS_ACCESS_KEY || path.join(os.homedir(), '.config/lms-shell/access-signing.pem');
const BRAND = path.join(path.dirname(fileURLToPath(import.meta.url)), '../shared/src/brand.rs');
const DOMAIN = (fs.readFileSync(BRAND, 'utf8').match(/ACCESS_CODE_DOMAIN: &str = "([^"]+)"/) || [])[1];
if (!DOMAIN) { console.error(`ACCESS_CODE_DOMAIN not found in ${BRAND}`); process.exit(2); }
const [arg, ...rest] = process.argv.slice(2);
const pubBytes = (k) => crypto.createPublicKey(k).export({ format: 'der', type: 'spki' }).subarray(-32);
const rustRows = (b) => [0, 16].map((i) => '    ' + [...b.subarray(i, i + 16)].map((x) => `0x${x.toString(16).padStart(2, '0')},`).join(' ')).join('\n');

if (!arg) { console.error('usage: access-code.mjs keygen | <username> [--days N]'); process.exit(2); }
if (arg === 'keygen') {
  if (fs.existsSync(KEY)) { console.error(`${KEY} exists; refusing to overwrite (every issued code depends on it).`); process.exit(1); }
  const { privateKey } = crypto.generateKeyPairSync('ed25519');
  fs.mkdirSync(path.dirname(KEY), { recursive: true, mode: 0o700 });
  fs.writeFileSync(KEY, privateKey.export({ format: 'pem', type: 'pkcs8' }), { mode: 0o600 });
  console.log(`private key: ${KEY} (back it up; losing it means no new codes)\n\nPaste this over PUBLIC_KEY in src-tauri/src/access.rs:\n\nconst PUBLIC_KEY: [u8; 32] = [\n${rustRows(pubBytes(privateKey))}\n];`);
  process.exit(0);
}
const user = arg.trim().toLowerCase();
const di = rest.indexOf('--days');
const days = di >= 0 ? Number(rest[di + 1]) : 365;
if (!user || !Number.isInteger(days) || days < 0) { console.error('need a username and a whole number of --days'); process.exit(2); }
const expiry = days === 0 ? 0 : Math.floor(Date.now() / 86_400_000) + days; // days since 1970-01-01, UTC
const key = crypto.createPrivateKey(fs.readFileSync(KEY));
const sig = crypto.sign(null, Buffer.from(`${DOMAIN}|${user}|${expiry}`), key).toString('base64url');
console.log(`username: ${user}\naccess code: ${expiry}.${sig}\nexpires: ${expiry ? new Date(expiry * 86_400_000).toISOString().slice(0, 10) : 'never'}`);
