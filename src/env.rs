use std::{
    any::Any,
    collections::HashMap,
    fmt, fs,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use crate::{
    error::EnvError,
    interpolator::EnvInterpolator,
    parser,
    rules::FieldValidator,
    secret::{FsSecretResolver, Secret, SecretResolver},
};

static GLOBAL_ENV: OnceLock<Env> = OnceLock::new();

/// Initializes the global environment store.
///
/// # Errors
/// Returns `EnvError::AlreadyInitialized` if the global store has already been initialized.
pub fn init(env: Env) -> Result<&'static Env, EnvError> {
    GLOBAL_ENV
        .set(env)
        .map_err(|_| EnvError::AlreadyInitialized)?;
    Ok(GLOBAL_ENV.get().expect("global env initialized"))
}

/// Initializes the global environment store if it has not already been set.
/// Returns a reference to the global store.
pub fn init_if_unset(env: Env) -> &'static Env {
    let _ = GLOBAL_ENV.set(env);
    GLOBAL_ENV.get().expect("global env initialized")
}

/// Retrieves a reference to the globally initialized `Env` container.
///
/// # Errors
/// Returns `EnvError::NotInitialized` if the global store has not been initialized.
pub fn current() -> Result<&'static Env, EnvError> {
    GLOBAL_ENV.get().ok_or(EnvError::NotInitialized)
}

/// Retrieves a typed reference to a validated variable from the global environment store.
///
/// # Errors
/// Returns `EnvError::NotInitialized` if the global store has not been initialized.
/// Returns `EnvError::NotFound` if the key does not exist.
/// Returns `EnvError::TypeMismatch` if the key cannot be downcast to `T`.
pub fn get<T: 'static>(key: &str) -> Result<&'static T, EnvError> {
    current()?.get::<T>(key)
}

/// Retrieves the raw string representation of a variable from the global environment store.
///
/// # Errors
/// Returns `EnvError::NotInitialized` if the global store has not been initialized.
/// Returns `EnvError::NotFound` if the key does not exist.
pub fn get_raw(key: &str) -> Result<&'static str, EnvError> {
    current()?.get_raw(key).ok_or_else(|| EnvError::NotFound {
        key: key.to_string(),
    })
}

/// Checks whether a variable exists in the global environment store.
pub fn contains(key: &str) -> bool {
    current().map(|env| env.contains(key)).unwrap_or(false)
}

/// Fluent configuration builder for cascading discovery, interpolation, and schema validation.
pub struct EnvBuilder {
    base_dir: PathBuf,
    active_env: Option<String>,
    override_system: bool,
    explicit_file: Option<PathBuf>,
    secret_resolver: Arc<dyn SecretResolver>,
    rules: Vec<(String, Box<dyn FieldValidator>)>,
}

impl Default for EnvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvBuilder {
    /// Creates a new `EnvBuilder` with default discovery settings.
    pub fn new() -> Self {
        Self {
            base_dir: PathBuf::from("."),
            active_env: None,
            override_system: false,
            explicit_file: None,
            secret_resolver: Arc::new(FsSecretResolver),
            rules: Vec::new(),
        }
    }

    /// Sets the base directory from which `.env` files are loaded.
    pub fn base_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.base_dir = path.into();
        self
    }

    /// Alias for `base_dir`.
    pub fn with_base_dir(self, path: impl Into<PathBuf>) -> Self {
        self.base_dir(path)
    }

    /// Sets the active environment name (e.g. `"development"`, `"production"`, `"test"`).
    pub fn env(mut self, env_name: impl Into<String>) -> Self {
        self.active_env = Some(env_name.into());
        self
    }

    /// Alias for `env`.
    pub fn with_env(self, env_name: impl Into<String>) -> Self {
        self.env(env_name)
    }

    /// Configures whether `.env` file values should override existing OS environment variables.
    pub fn override_system(mut self, override_system: bool) -> Self {
        self.override_system = override_system;
        self
    }

    /// Directs the loader to read an explicit `.env` file instead of cascading.
    pub fn with_file(mut self, file_path: impl Into<PathBuf>) -> Self {
        self.explicit_file = Some(file_path.into());
        self
    }

    /// Configures a custom secret resolver for resolving external secrets.
    pub fn secret_resolver(mut self, resolver: impl SecretResolver + 'static) -> Self {
        self.secret_resolver = Arc::new(resolver);
        self
    }

    /// Registers a validation rule for the specified environment variable name.
    pub fn rule<T: Send + Sync + 'static>(
        mut self,
        key: impl Into<String>,
        rule: crate::rules::Rule<T>,
    ) -> Self {
        self.rules.push((key.into(), Box::new(rule)));
        self
    }

    /// Resolves the active environment name from explicit config, `PORIBESH_ENV`, `APP_ENV`, or defaults to `"development"`.
    pub fn resolve_active_env(&self) -> String {
        if let Some(env_name) = &self.active_env {
            return env_name.clone();
        }

        if let Ok(env_name) = std::env::var("PORIBESH_ENV")
            && !env_name.trim().is_empty()
        {
            return env_name.trim().to_string();
        }

        if let Ok(env_name) = std::env::var("APP_ENV")
            && !env_name.trim().is_empty()
        {
            return env_name.trim().to_string();
        }

        "development".to_string()
    }

    /// Returns the cascading list of `.env` file paths in ascending priority order.
    pub fn get_cascaded_paths(&self) -> Vec<PathBuf> {
        if let Some(file_path) = &self.explicit_file {
            let target_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                self.base_dir.join(file_path)
            };
            return vec![target_path];
        }

        let env_name = self.resolve_active_env();
        let is_test = env_name == "test" || env_name == "testing";

        let mut paths = Vec::new();
        paths.push(self.base_dir.join(".env"));

        if !env_name.is_empty() {
            paths.push(self.base_dir.join(format!(".env.{env_name}")));
        }

        if !is_test {
            paths.push(self.base_dir.join(".env.local"));
        }

        if !env_name.is_empty() {
            paths.push(self.base_dir.join(format!(".env.{env_name}.local")));
        }

        paths
    }

    /// Discovers, reads, merges, and interpolates environment variables into a raw map.
    pub fn load_raw_map(&self) -> Result<HashMap<String, String>, EnvError> {
        let mut file_map = HashMap::new();

        if let Some(explicit_file) = &self.explicit_file {
            let target_path = if explicit_file.is_absolute() {
                explicit_file.clone()
            } else {
                self.base_dir.join(explicit_file)
            };

            let contents = fs::read_to_string(&target_path).map_err(|err| {
                EnvError::Io(std::io::Error::new(
                    err.kind(),
                    format!("failed to read `{}`: {err}", target_path.display()),
                ))
            })?;

            let entries = parser::parse_to_map(&contents)?;
            file_map.extend(entries);
        } else {
            let paths = self.get_cascaded_paths();
            for path in paths {
                if path.is_file() {
                    let contents = match fs::read_to_string(&path) {
                        Ok(text) => text,
                        Err(err) => {
                            return Err(EnvError::Io(std::io::Error::new(
                                err.kind(),
                                format!("failed to read `{}`: {err}", path.display()),
                            )));
                        }
                    };
                    let entries = parser::parse_to_map(&contents)?;
                    file_map.extend(entries);
                }
            }
        }

        let mut merged = HashMap::new();
        let system_vars: HashMap<String, String> = std::env::vars().collect();

        if self.override_system {
            merged.extend(system_vars);
            merged.extend(file_map);
        } else {
            merged.extend(file_map);
            merged.extend(system_vars);
        }

        let interpolator = EnvInterpolator::new(&self.base_dir, self.secret_resolver.as_ref());
        interpolator.interpolate(&merged)
    }

    /// Ingests and interpolates environment variables directly from a raw string.
    pub fn load_from_str(&self, contents: &str) -> Result<HashMap<String, String>, EnvError> {
        let file_map = parser::parse_to_map(contents)?;
        let mut merged = HashMap::new();
        let system_vars: HashMap<String, String> = std::env::vars().collect();

        if self.override_system {
            merged.extend(system_vars);
            merged.extend(file_map);
        } else {
            merged.extend(file_map);
            merged.extend(system_vars);
        }

        let interpolator = EnvInterpolator::new(&self.base_dir, self.secret_resolver.as_ref());
        interpolator.interpolate(&merged)
    }

    /// Discovers, reads, merges, interpolates, and validates environment variables.
    pub fn load(&self) -> Result<Env, EnvError> {
        let raw_map = self.load_raw_map()?;
        self.validate_owned(raw_map)
    }

    /// Loads and validates configuration using the discovery settings from `loader`.
    pub fn load_and_validate(&self, loader: &EnvBuilder) -> Result<Env, EnvError> {
        let raw_map = loader.load_raw_map()?;
        self.validate_owned(raw_map)
    }

    /// Ingests and validates directly from a raw `.env` string without accessing the filesystem.
    pub fn from_str(&self, contents: &str) -> Result<Env, EnvError> {
        let interpolated = self.load_from_str(contents)?;
        self.validate_owned(interpolated)
    }

    /// Parses raw `.env` contents and validates the schema. Alias for `from_str`.
    pub fn parse_and_validate(&self, contents: &str) -> Result<Env, EnvError> {
        self.from_str(contents)
    }

    /// Ingests and validates from a specific file path.
    pub fn from_file(&self, path: impl Into<PathBuf>) -> Result<Env, EnvError> {
        let mut cloned = Self {
            base_dir: self.base_dir.clone(),
            active_env: self.active_env.clone(),
            override_system: self.override_system,
            explicit_file: Some(path.into()),
            secret_resolver: self.secret_resolver.clone(),
            rules: Vec::new(),
        };
        // Move rules
        cloned.rules = Vec::with_capacity(self.rules.len());
        // Since FieldValidator doesn't clone easily, we use load with configured explicit file
        let target_path = cloned.explicit_file.as_ref().unwrap();
        let actual_path = if target_path.is_absolute() {
            target_path.clone()
        } else {
            self.base_dir.join(target_path)
        };
        let contents = fs::read_to_string(&actual_path).map_err(|err| {
            EnvError::Io(std::io::Error::new(
                err.kind(),
                format!("failed to read `{}`: {err}", actual_path.display()),
            ))
        })?;
        self.from_str(&contents)
    }

    /// Validates the provided raw key-value mapping against registered schema rules.
    pub fn validate(&self, raw_map: &HashMap<String, String>) -> Result<Env, EnvError> {
        self.validate_arc(Arc::new(raw_map.clone()))
    }

    /// Validates an owned raw key-value mapping against registered schema rules.
    pub fn validate_owned(&self, raw_map: HashMap<String, String>) -> Result<Env, EnvError> {
        self.validate_arc(Arc::new(raw_map))
    }

    /// Validates an `Arc`-shared raw key-value mapping against registered schema rules.
    pub fn validate_arc(&self, raw_map: Arc<HashMap<String, String>>) -> Result<Env, EnvError> {
        let mut errors = Vec::new();
        let mut typed_values: HashMap<String, Box<dyn Any + Send + Sync>> =
            HashMap::with_capacity(self.rules.len());

        for (key, validator) in &self.rules {
            match validator.validate(key, &raw_map) {
                Ok(boxed_value) => {
                    typed_values.insert(key.clone(), boxed_value);
                }
                Err(validation_error) => {
                    errors.push(validation_error);
                }
            }
        }

        if !errors.is_empty() {
            Err(EnvError::ValidationFailed { errors })
        } else {
            Ok(Env::new(raw_map, typed_values))
        }
    }
}

/// An immutable, thread-safe container holding validated typed environment variables.
pub struct Env {
    raw_values: Arc<HashMap<String, String>>,
    typed_values: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl fmt::Debug for Env {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut keys: Vec<&String> = self.raw_values.keys().collect();
        keys.sort();
        formatter.debug_struct("Env").field("keys", &keys).finish()
    }
}

impl std::str::FromStr for Env {
    type Err = EnvError;

    fn from_str(contents: &str) -> Result<Self, Self::Err> {
        Self::builder().from_str(contents)
    }
}

impl Env {
    /// Constructs a new `Env` instance with the given raw and typed mappings.
    pub fn new(
        raw_values: impl Into<Arc<HashMap<String, String>>>,
        typed_values: HashMap<String, Box<dyn Any + Send + Sync>>,
    ) -> Self {
        Self {
            raw_values: raw_values.into(),
            typed_values,
        }
    }

    /// Creates a new fluent `EnvBuilder` for configuring rules and loading configuration.
    pub fn builder() -> EnvBuilder {
        EnvBuilder::new()
    }

    /// Alias for `builder` when focusing on schema rules.
    pub fn rules() -> EnvBuilder {
        EnvBuilder::new()
    }

    /// Creates a new `EnvBuilder` for discovering and reading `.env` files.
    pub fn loader() -> EnvBuilder {
        EnvBuilder::new()
    }

    /// Loads environment variables using default cascading discovery without rules.
    pub fn load() -> Result<Self, EnvError> {
        Self::builder().load()
    }

    /// Ingests environment variables from an explicit file path without rules.
    pub fn from_file(path: impl Into<PathBuf>) -> Result<Self, EnvError> {
        Self::builder().with_file(path).load()
    }

    /// Parses raw `.env` content directly into a key-value HashMap.
    pub fn parse_to_map(contents: &str) -> Result<HashMap<String, String>, EnvError> {
        parser::parse_to_map(contents)
    }

    /// Retrieves a typed reference to a validated environment variable.
    ///
    /// # Errors
    /// Returns `EnvError::NotFound` if the key was not defined or validated.
    /// Returns `EnvError::TypeMismatch` if the key cannot be downcast to `T`.
    pub fn get<T: 'static>(&self, key: &str) -> Result<&T, EnvError> {
        match self.typed_values.get(key) {
            Some(boxed) => match boxed.downcast_ref::<T>() {
                Some(value) => Ok(value),
                None => Err(EnvError::TypeMismatch {
                    key: key.to_string(),
                    expected: std::any::type_name::<T>(),
                }),
            },
            None => Err(EnvError::NotFound {
                key: key.to_string(),
            }),
        }
    }

    /// Retrieves a reference to a secret variable wrapped in `Secret<T>`.
    pub fn get_secret<T: 'static>(&self, key: &str) -> Result<&Secret<T>, EnvError> {
        self.get::<Secret<T>>(key)
    }

    /// Retrieves the raw string representation of an environment variable.
    pub fn get_raw(&self, key: &str) -> Option<&str> {
        self.raw_values.get(key).map(String::as_str)
    }

    /// Checks whether an environment variable exists in the validated set or raw map.
    pub fn contains(&self, key: &str) -> bool {
        self.typed_values.contains_key(key) || self.raw_values.contains_key(key)
    }

    /// Returns a reference to all raw environment key-value pairs.
    pub fn raw_map(&self) -> &HashMap<String, String> {
        &self.raw_values
    }

    /// Exports all raw environment variables into the process-level environment (`std::env`).
    ///
    /// # Safety
    /// Modifying process environment variables via POSIX `setenv` is inherently not thread-safe.
    /// This method must only be called during early single-threaded application bootstrap before
    /// any worker threads or async runtimes have been spawned.
    pub unsafe fn export_to_process(&self) {
        for (key, value) in self.raw_values.iter() {
            // Safety: caller guarantees single-threaded boot phase
            unsafe {
                std::env::set_var(key, value);
            }
        }
    }
}

#[cfg(feature = "serde")]
mod de {
    use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

    use super::*;

    impl Env {
        /// Deserializes the raw environment variables into a strongly-typed struct.
        pub fn deserialize<'de, T: serde::Deserialize<'de>>(&self) -> Result<T, EnvError> {
            T::deserialize(EnvDeserializer {
                raw_map: &self.raw_values,
            })
            .map_err(|err| EnvError::Deserialize(err.to_string()))
        }
    }

    struct EnvDeserializer<'a> {
        raw_map: &'a HashMap<String, String>,
    }

    impl<'de, 'a> Deserializer<'de> for EnvDeserializer<'a> {
        type Error = de::value::Error;

        fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            visitor.visit_map(EnvMapAccess {
                iter: self.raw_map.iter(),
                current_value: None,
            })
        }

        fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.deserialize_any(visitor)
        }

        fn deserialize_struct<V: Visitor<'de>>(
            self,
            name: &'static str,
            fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Self::Error> {
            let _ = (name, fields);
            self.deserialize_any(visitor)
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct enum identifier ignored_any
        }
    }

    struct EnvMapAccess<'a> {
        iter: std::collections::hash_map::Iter<'a, String, String>,
        current_value: Option<&'a str>,
    }

    impl<'de, 'a> MapAccess<'de> for EnvMapAccess<'a> {
        type Error = de::value::Error;

        fn next_key_seed<K: DeserializeSeed<'de>>(
            &mut self,
            seed: K,
        ) -> Result<Option<K::Value>, Self::Error> {
            if let Some((key, value)) = self.iter.next() {
                self.current_value = Some(value.as_str());
                seed.deserialize(de::IntoDeserializer::into_deserializer(key.as_str()))
                    .map(Some)
            } else {
                Ok(None)
            }
        }

        fn next_value_seed<V: DeserializeSeed<'de>>(
            &mut self,
            seed: V,
        ) -> Result<V::Value, Self::Error> {
            let value_str = self
                .current_value
                .take()
                .expect("next_value_seed called before next_key_seed");
            seed.deserialize(StringValueDeserializer { value: value_str })
        }
    }

    struct StringValueDeserializer<'a> {
        value: &'a str,
    }

    impl<'de, 'a> Deserializer<'de> for StringValueDeserializer<'a> {
        type Error = de::value::Error;

        fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            visitor.visit_str(self.value)
        }

        fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            match self.value.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => visitor.visit_bool(true),
                "false" | "0" | "no" | "off" => visitor.visit_bool(false),
                _ => Err(de::Error::custom(format!(
                    "invalid boolean value `{}`",
                    self.value
                ))),
            }
        }

        fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<i8>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_i8(v))
        }

        fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<i16>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_i16(v))
        }

        fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<i32>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_i32(v))
        }

        fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<i64>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_i64(v))
        }

        fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<u8>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_u8(v))
        }

        fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<u16>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_u16(v))
        }

        fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<u32>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_u32(v))
        }

        fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<u64>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_u64(v))
        }

        fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<f32>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_f32(v))
        }

        fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            self.value
                .trim()
                .parse::<f64>()
                .map_err(de::Error::custom)
                .and_then(|v| visitor.visit_f64(v))
        }

        fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            if self.value.trim().is_empty() {
                visitor.visit_none()
            } else {
                visitor.visit_some(self)
            }
        }

        fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            let parts: Vec<&str> = self
                .value
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            visitor.visit_seq(SeqSliceAccess {
                iter: parts.into_iter(),
            })
        }

        serde::forward_to_deserialize_any! {
            i128 u128 char str string bytes byte_buf unit unit_struct
            newtype_struct tuple tuple_struct map struct enum identifier ignored_any
        }
    }

    struct SeqSliceAccess<'a> {
        iter: std::vec::IntoIter<&'a str>,
    }

    impl<'de, 'a> SeqAccess<'de> for SeqSliceAccess<'a> {
        type Error = de::value::Error;

        fn next_element_seed<T: DeserializeSeed<'de>>(
            &mut self,
            seed: T,
        ) -> Result<Option<T::Value>, Self::Error> {
            if let Some(item) = self.iter.next() {
                seed.deserialize(StringValueDeserializer { value: item })
                    .map(Some)
            } else {
                Ok(None)
            }
        }
    }
}
