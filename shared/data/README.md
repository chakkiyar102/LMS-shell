# shared/data

Data files embedded into the `shared` crate with `include_str!`.

| File | What it is |
|---|---|
| `disposable_domains.txt` | Temporary and disposable email domains refused for the profile's work email. |
| `allowed_domains.txt` | Domains always accepted, checked before the disposable list. |

## disposable_domains.txt

- Source: https://github.com/disposable-email-domains/disposable-email-domains
- File: `disposable_email_blocklist.conf`
- Commit: `fa960072882ed3e629ffdc064d2ccb9d5b461c2f`
- Fetched: 2026-10-27
- Licence: CC0 1.0 Universal (public domain dedication)
- Domains: 9963

To update, fetch the file at a newer commit:

```sh
SHA=<commit>
curl -sf "https://raw.githubusercontent.com/disposable-email-domains/disposable-email-domains/$SHA/disposable_email_blocklist.conf" -o /tmp/list.conf
```

Keep the `#` header of `disposable_domains.txt`, replace the domain lines with the fetched file, update the commit, date and count above and in the header, then run `cargo test -p shared`. To accept a domain the list wrongly blocks, add it to `allowed_domains.txt` instead of editing the vendored lines.
