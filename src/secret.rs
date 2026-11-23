use std::{collections::HashMap, fmt, fs, path::Path};

use crate::error::EnvError;

/// A wrapper type for sensitive values (passwords, tokens, API keys).
///
/// `Secret<T>` prevents sensitive values from being accidentally printed in
/// logs, debug traces, or error diagnostics by masking them as `[REDACTED]`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Secret<T> {
    inner: T,
}

impl<T> Secret<T> {
    /// Wraps a value in a secret container.
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Exposes a reference to the sensitive value.
    pub fn expose_secret(&self) -> &T {
        &self.inner
    }

    /// Consumes the wrapper and returns the underlying value.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

#[cfg(feature = "serde")]
impl<T> serde::Serialize for Secret<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for Secret<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::new)
    }
}

/// Defines the seam for resolving sensitive values from external sources or URI schemes.
pub trait SecretResolver: Send + Sync {
    /// Resolves a secret target relative to a base directory.
    fn resolve(&self, target: &str, base_path: &Path) -> Result<String, EnvError>;
}

/// Default filesystem secret adapter for resolving file paths (e.g. `file:<path>`).
#[derive(Debug, Default, Clone, Copy)]
pub struct FsSecretResolver;

impl SecretResolver for FsSecretResolver {
    fn resolve(&self, target: &str, base_path: &Path) -> Result<String, EnvError> {
        let relative_or_absolute = Path::new(target.trim());
        let target_path = if relative_or_absolute.is_absolute() {
            relative_or_absolute.to_path_buf()
        } else {
            base_path.join(relative_or_absolute)
        };

        match fs::read_to_string(&target_path) {
            Ok(contents) => {
                let trimmed = contents
                    .strip_suffix("\r\n")
                    .or_else(|| contents.strip_suffix('\n'))
                    .unwrap_or(&contents);
                Ok(trimmed.to_string())
            }
            Err(err) => Err(EnvError::Resolver {
                key: target.to_string(),
                message: format!(
                    "unable to read secret file `{}`: {err}",
                    target_path.display()
                ),
            }),
        }
    }
}

/// In-memory secret adapter for hermetic unit testing without physical filesystem access.
#[derive(Debug, Default, Clone)]
pub struct MemorySecretResolver {
    secrets: HashMap<String, String>,
}

impl MemorySecretResolver {
    /// Creates a new empty in-memory secret resolver.
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }

    /// Inserts a secret value mapped to a path or key.
    pub fn with_secret(mut self, path: impl Into<String>, secret: impl Into<String>) -> Self {
        self.secrets.insert(path.into(), secret.into());
        self
    }
}

impl SecretResolver for MemorySecretResolver {
    fn resolve(&self, target: &str, base_path: &Path) -> Result<String, EnvError> {
        let _ = base_path;
        let clean_target = target.trim();
        self.secrets
            .get(clean_target)
            .cloned()
            .ok_or_else(|| EnvError::Resolver {
                key: target.to_string(),
                message: format!("secret `{target}` not found in memory resolver"),
            })
    }
}
