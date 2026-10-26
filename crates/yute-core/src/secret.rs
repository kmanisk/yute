//! Owned secrets erase their allocation on drop and never print their contents.
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SecretString(Zeroizing<String>);
impl SecretString {
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(Zeroizing::new(value))
    }
}
impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}
impl std::ops::Deref for SecretString {
    type Target = str;
    fn deref(&self) -> &str {
        self.expose()
    }
}
impl AsRef<str> for SecretString {
    fn as_ref(&self) -> &str {
        self.expose()
    }
}
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
impl Serialize for SecretString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}
impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(String::deserialize(deserializer)?.into())
    }
}
