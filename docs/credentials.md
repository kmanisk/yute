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
blocking workers. Existing keyring-backed files require their original keyring;
an unavailable keyring never causes them to be overwritten with a new key.

For a new store without a usable keyring, Yute creates a random 32-byte keyfile
at `~/.local/share/yute/master.key`, mode 0600. This is encrypted credential
storage with a separate local key, not protection against someone who can read
both files. Set `YUTE_KEYFILE` to select another private keyfile. Alternatively,
set `YUTE_PASSPHRASE_FILE` to a private file containing the exact passphrase bytes.
Argon2id derives the AES key using a random 128-bit salt in the authenticated
header. Preserve the passphrase file exactly, including any final newline.
No credentials silently fall back to plaintext. Key material and owned secret
strings use zeroizing allocations, and Debug redacts them.

Legacy `~/.config/yute/auth.json`, `~/.config/ytui/auth.json`, and configured
Netscape cookie files are merged without replacing existing accounts/settings.
The old machine-derived AES format is accepted only for migration. The encrypted
copy is read and authenticated before old files are unlinked. Physical secure
erasure cannot be guaranteed on SSDs, copy-on-write filesystems, snapshots, or
backups. Restart older running clients so they cannot recreate legacy files.
Yute no longer automatically extracts browser cookies into plaintext files.

Automated tests use explicit temporary stores and fixture keys. They cover
round-trip, wrong keys, header/ciphertext tampering, nonce uniqueness, mode 0600,
redacted Debug, and migration with destination preservation. Keychain and
Credential Manager behavior requires testing on their respective operating systems.
