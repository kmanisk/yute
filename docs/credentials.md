# Credential storage

The core stores accounts, OAuth access/refresh tokens, cookie jars, and visitor
state in `$XDG_DATA_HOME/yute/credentials.enc` (normally
`~/.local/share/yute/credentials.enc`). AES-256-GCM authenticates the complete
versioned header, key source, salt, nonce, and ciphertext. Every write gets a
fresh 96-bit OS-random nonce. Temporary files use mode 0600, are fsynced, then
renamed within the destination directory; the directory is fsynced too.
A process lock serializes readers/migrations/updates between running clients.

The master key normally lives in the OS keyring: Secret Service on Linux,
Keychain on macOS, or Credential Manager on Windows. Access is lazy and runs on
blocking workers. A delayed unlock is reported without blocking input, and quit
does not wait indefinitely for an OS keyring prompt. Existing keyring-backed files require their original keyring;
an unavailable keyring never causes them to be overwritten with a new key.

New stores require an OS-keyring master key. A missing/locked/unavailable keyring
is an error; no automatic local keyfile or passphrase fallback is created.
Existing encrypted stores backed by a keyfile or passphrase retain their original
key source for compatibility. `YUTE_KEYFILE` and `YUTE_PASSPHRASE_FILE` only
select keys for those existing stores. Migration of those stores to an OS-keyring
master key remains outstanding; preserve their original keys until that happens.
No credentials silently fall back to plaintext. Key material and owned secret
strings use zeroizing allocations, and Debug redacts them.

Legacy `~/.config/yute/auth.json` and `~/.config/ytui/auth.json` are merged
without replacing existing accounts/settings. Normal startup leaves legacy
Netscape cookie files untouched. Previously encrypted cookie jars remain opaque
compatibility data; they never authorize requests or create a signed-in profile.
Cookie-only profiles become Needs re-login and require TV OAuth.
The old machine-derived AES format is accepted only for migration. The encrypted
copy is read and authenticated before old files are unlinked. Physical secure
erasure cannot be guaranteed on SSDs, copy-on-write filesystems, snapshots, or
backups. Restart older running clients so they cannot recreate legacy files.
Yute never passes cookies to InnerTube, MPV, libmpv or yt-dlp.

Automated tests use explicit temporary stores and fixture keys. They cover
round-trip, wrong keys, header/ciphertext tampering, nonce uniqueness, mode 0600,
redacted Debug, and migration with destination preservation. Keychain and
Credential Manager behavior requires testing on their respective operating systems.
