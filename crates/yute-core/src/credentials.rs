//! Versioned AEAD storage. Call filesystem/keyring APIs on a blocking worker.
use crate::backend::auth::AuthStorage;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use zeroize::Zeroizing;

const PREFIX: &str = "YUTE_ENC_V2:";
const MAGIC: &[u8] = b"YUTE\0\x02";
const HEADER: usize = 6 + 1 + 16 + 12;
static WRITE_LOCK: Mutex<()> = Mutex::new(());
static KEYS: OnceLock<Mutex<HashMap<PathBuf, Arc<MasterKey>>>> = OnceLock::new();

struct MasterKey {
    bytes: Zeroizing<[u8; 32]>,
    source: u8,
    salt: [u8; 16],
}
pub struct CredentialStore {
    path: PathBuf,
    key: Arc<MasterKey>,
}
impl std::fmt::Debug for CredentialStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CredentialStore([REDACTED])")
    }
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::config::resolve_path("~/.local/share"))
        .join("yute")
}
pub fn path() -> PathBuf {
    data_dir().join("credentials.enc")
}
fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|_| anyhow::anyhow!("OS random source unavailable"))?;
    Ok(bytes)
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Credential path has no parent")?;
    private_dir(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|_| anyhow::anyhow!("Could not atomically save credentials"))?;
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
fn private_read(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        bail!("Secret key must be a regular file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            bail!("Secret key/passphrase file must have mode 0600");
        }
    }
    Ok(Zeroizing::new(fs::read(path)?))
}
fn keyfile() -> Result<Zeroizing<[u8; 32]>> {
    let file = std::env::var_os("YUTE_KEYFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir().join("master.key"));
    let bytes = private_read(&file)?;
    let key = <[u8; 32]>::try_from(bytes.as_slice())
        .map_err(|_| anyhow::anyhow!("Keyfile must contain exactly 32 random bytes"))?;
    Ok(Zeroizing::new(key))
}
fn keyring_key(create: bool) -> Result<Zeroizing<[u8; 32]>> {
    let entry = keyring::Entry::new("yute", "credential-master-v2")
        .map_err(|_| anyhow::anyhow!("OS keyring unavailable"))?;
    let bytes = match entry.get_secret() {
        Ok(bytes) => Zeroizing::new(bytes),
        Err(keyring::Error::NoEntry) if create => {
            let key = Zeroizing::new(random::<32>()?);
            entry
                .set_secret(key.as_ref())
                .map_err(|_| anyhow::anyhow!("OS keyring unavailable"))?;
            Zeroizing::new(key.to_vec())
        }
        Err(_) => bail!("OS keyring locked or unavailable; unlock it and retry"),
    };
    Ok(Zeroizing::new(
        <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("Invalid keyring master key"))?,
    ))
}
fn passphrase_key(salt: &[u8; 16]) -> Result<Zeroizing<[u8; 32]>> {
    let file = std::env::var_os("YUTE_PASSPHRASE_FILE")
        .context("Set YUTE_PASSPHRASE_FILE to a private passphrase file")?;
    let password = private_read(Path::new(&file))?;
    if password.is_empty() {
        bail!("Empty passphrase is not allowed");
    }
    let mut key = Zeroizing::new([0; 32]);
    argon2::Argon2::default()
        .hash_password_into(&password, salt, key.as_mut())
        .map_err(|_| anyhow::anyhow!("Passphrase key derivation failed"))?;
    Ok(key)
}
impl CredentialStore {
    pub fn open(path: PathBuf) -> Result<Self> {
        let keys = KEYS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut keys = keys.lock().unwrap();
        if let Some(key) = keys.get(&path) {
            return Ok(Self {
                path,
                key: key.clone(),
            });
        }
        let key = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            let payload = decode(&raw)?;
            let source = payload[6];
            let mut salt = [0; 16];
            salt.copy_from_slice(&payload[7..23]);
            let bytes = match source {
                0 => keyring_key(false)?,
                1 => keyfile()?,
                2 => passphrase_key(&salt)?,
                _ => bail!("Unsupported credential key source"),
            };
            MasterKey {
                bytes,
                source,
                salt,
            }
        } else {
            // New stores require the OS keyring. Legacy encrypted stores retain
            // their original key source so upgrades never destroy credentials.
            MasterKey {
                bytes: keyring_key(true)?,
                source: 0,
                salt: [0; 16],
            }
        };
        let key = Arc::new(key);
        keys.insert(path.clone(), key.clone());
        Ok(Self { path, key })
    }
    #[cfg(test)]
    fn with_key(path: PathBuf, key: [u8; 32]) -> Self {
        Self {
            path,
            key: Arc::new(MasterKey {
                bytes: Zeroizing::new(key),
                source: 1,
                salt: [0; 16],
            }),
        }
    }
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String> {
        let nonce = random::<12>()?;
        let mut header = MAGIC.to_vec();
        header.push(self.key.source);
        header.extend_from_slice(&self.key.salt);
        header.extend_from_slice(&nonce);
        let cipher = Aes256Gcm::new_from_slice(self.key.bytes.as_ref())
            .map_err(|_| anyhow::anyhow!("Invalid master key"))?;
        let encrypted = cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: &header,
                },
            )
            .map_err(|_| anyhow::anyhow!("Credential encryption failed"))?;
        header.extend_from_slice(&encrypted);
        Ok(format!("{PREFIX}{}", hex::encode(header)))
    }
    pub fn decrypt(&self, raw: &str) -> Result<Zeroizing<Vec<u8>>> {
        let payload = decode(raw)?;
        let nonce =
            <[u8; 12]>::try_from(&payload[23..35]).map_err(|_| anyhow::anyhow!("Invalid nonce"))?;
        let cipher = Aes256Gcm::new_from_slice(self.key.bytes.as_ref())
            .map_err(|_| anyhow::anyhow!("Invalid master key"))?;
        let plaintext = cipher
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: &payload[HEADER..],
                    aad: &payload[..HEADER],
                },
            )
            .map_err(|_| {
                anyhow::anyhow!("Credentials could not be authenticated; wrong key or damaged file")
            })?;
        Ok(Zeroizing::new(plaintext))
    }
    pub fn load(&self) -> Result<AuthStorage> {
        if !self.path.exists() {
            return Ok(AuthStorage::default());
        }
        Ok(serde_json::from_slice(
            &self.decrypt(&fs::read_to_string(&self.path)?)?,
        )?)
    }
    pub fn save(&self, storage: &AuthStorage) -> Result<()> {
        let bytes = Zeroizing::new(serde_json::to_vec(storage)?);
        atomic_write(&self.path, self.encrypt(&bytes)?.as_bytes())
    }
}
fn decode(raw: &str) -> Result<Vec<u8>> {
    let hex = raw
        .strip_prefix(PREFIX)
        .context("Unsupported credential header")?;
    let payload =
        hex::decode(hex.trim()).map_err(|_| anyhow::anyhow!("Invalid credential encoding"))?;
    if payload.len() < HEADER + 16 || &payload[..MAGIC.len()] != MAGIC {
        bail!("Invalid credential version or truncated file");
    }
    Ok(payload)
}
fn process_lock() -> Result<fs::File> {
    private_dir(&data_dir())?;
    let mut options = fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(data_dir().join("credentials.lock"))?;
    file.lock()?;
    Ok(file)
}
pub fn load() -> Result<AuthStorage> {
    let path = path();
    if !path.exists() {
        return Ok(AuthStorage::default());
    }
    let _guard = WRITE_LOCK.lock().unwrap();
    let _file = process_lock()?;
    CredentialStore::open(path)?.load()
}
pub fn update<F: FnOnce(&mut AuthStorage) -> Result<()>>(update: F) -> Result<()> {
    let _guard = WRITE_LOCK.lock().unwrap();
    let _file = process_lock()?;
    let store = CredentialStore::open(path())?;
    let mut storage = store.load()?;
    update(&mut storage)?;
    store.save(&storage)
}
/// Migrate only after the destination has been written and authenticated.
pub fn migrate(legacy_paths: &[PathBuf], cookie_paths: &[PathBuf]) -> Result<AuthStorage> {
    let _guard = WRITE_LOCK.lock().unwrap();
    let _file = process_lock()?;
    let exists = path().exists();
    if !exists && !legacy_paths.iter().chain(cookie_paths).any(|p| p.is_file()) {
        return Ok(AuthStorage::default());
    }
    let store = CredentialStore::open(path())?;
    migrate_into(&store, legacy_paths, cookie_paths)
}
fn migrate_into(
    store: &CredentialStore,
    legacy_paths: &[PathBuf],
    cookie_paths: &[PathBuf],
) -> Result<AuthStorage> {
    let exists = store.path.exists();
    let mut storage = store.load()?;
    let mut migrated = Vec::new();
    for legacy in legacy_paths {
        if !legacy.is_file() || *legacy == store.path {
            continue;
        }
        let raw = Zeroizing::new(fs::read_to_string(legacy)?);
        let bytes = if raw.starts_with("YTUI_ENC_V1:") {
            Zeroizing::new(crate::backend::auth::decrypt_legacy_auth_data(&raw)?)
        } else {
            Zeroizing::new(raw.as_bytes().to_vec())
        };
        let old: AuthStorage = serde_json::from_slice(&bytes)
            .context("Legacy credentials cannot be read; original retained")?;
        for (id, account) in old.accounts {
            storage.accounts.entry(id).or_insert(account);
        }
        if !exists && storage.active_account.is_none() {
            storage.active_account = old.active_account;
            storage.visitor_data = old.visitor_data;
        }
        migrated.push(legacy.clone());
    }
    for cookie in cookie_paths {
        if !cookie.is_file() {
            continue;
        }
        let raw = Zeroizing::new(fs::read_to_string(cookie)?);
        storage
            .cookie_jars
            .entry(cookie_key(cookie))
            .or_insert_with(|| raw.as_str().into());
        migrated.push(cookie.clone());
    }
    if !migrated.is_empty() {
        store.save(&storage)?;
        let _verified = store.load()?;
        for file in migrated {
            fs::remove_file(file)
                .context("Credentials migrated but legacy file could not be removed")?;
        }
    }
    Ok(storage)
}
pub fn cookie_key(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(path.to_string_lossy().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> AuthStorage {
        let mut storage = AuthStorage::default();
        storage.accounts.insert(
            "fixture-account".into(),
            crate::backend::auth::AuthAccount {
                access_token: "access-fixture".into(),
                refresh_token: "refresh-fixture".into(),
                name: "Example".into(),
                ..Default::default()
            },
        );
        storage.active_account = Some("fixture-account".into());
        storage
    }
    #[test]
    fn round_trip_is_private_atomic_and_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::with_key(dir.path().join("credentials.enc"), [7; 32]);
        store.save(&fixture()).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded.accounts["fixture-account"].access_token.expose(),
            "access-fixture"
        );
        assert!(!fs::read_to_string(&store.path)
            .unwrap()
            .contains("access-fixture"));
        assert!(!format!("{loaded:?}").contains("access-fixture"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&store.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn modified_header_ciphertext_and_wrong_key_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::with_key(dir.path().join("credentials.enc"), [7; 32]);
        let encoded = store.encrypt(b"fixture").unwrap();
        let bytes = decode(&encoded).unwrap();
        for index in [0, 5, 6, 8, 23, 34, HEADER, bytes.len() - 1] {
            let mut changed = bytes.clone();
            changed[index] ^= 1;
            assert!(store
                .decrypt(&format!("{PREFIX}{}", hex::encode(changed)))
                .is_err());
        }
        let wrong = CredentialStore::with_key(store.path.clone(), [8; 32]);
        assert!(wrong.decrypt(&encoded).is_err());
    }
    #[test]
    fn every_write_uses_a_fresh_nonce() {
        let store = CredentialStore::with_key(PathBuf::new(), [7; 32]);
        let mut nonces = std::collections::HashSet::new();
        for _ in 0..128 {
            let payload = decode(&store.encrypt(b"same plaintext").unwrap()).unwrap();
            assert!(nonces.insert(payload[23..35].to_vec()));
        }
    }
    #[test]
    fn migration_verifies_before_removing_sources_and_preserves_destination() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::with_key(dir.path().join("credentials.enc"), [7; 32]);
        let auth = dir.path().join("auth.json");
        let cookie = dir.path().join("cookies.txt");
        let config = dir.path().join("config.toml");
        fs::write(&config, "current config").unwrap();
        fs::write(&auth, serde_json::to_vec(&fixture()).unwrap()).unwrap();
        fs::write(&cookie,"# Netscape HTTP Cookie File\n.youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tcookie-fixture\n").unwrap();
        let migrated = migrate_into(&store, &[auth.clone()], &[cookie.clone()]).unwrap();
        assert!(!auth.exists() && !cookie.exists());
        assert_eq!(migrated.accounts.len(), 1);
        assert_eq!(migrated.cookie_jars.len(), 1);
        assert_eq!(fs::read_to_string(&config).unwrap(), "current config");
        let mut old = fixture();
        old.accounts.get_mut("fixture-account").unwrap().name = "Outdated".into();
        fs::write(&auth, serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(
            migrate_into(&store, &[auth.clone()], &[]).unwrap().accounts["fixture-account"].name,
            "Example"
        );
        fs::write(&auth, "not credentials").unwrap();
        assert!(migrate_into(&store, &[auth.clone()], &[]).is_err());
        assert!(auth.exists());
        assert_eq!(store.load().unwrap().accounts.len(), 1);
    }
}
