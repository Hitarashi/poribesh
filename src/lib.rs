//! Environment variable loading, cascading, expansion, and schema validation.
//!
//! # Architecture & Features
//! - **Cascading File Discovery**: Supports `.env.{env}.local` -> `.env.local` -> `.env.{env}` -> `.env`.
//! - **Variable Interpolation**: Expands `${VAR}`, `$VAR`, and fallbacks `${VAR:-default}` with cycle detection.
//! - **Container Secrets**: Built-in `file:<path>` prefix resolver for Docker and Kubernetes secrets.
//! - **Schema Validation**: Fluent rules for `string`, `integer`, `float`, `boolean`, `port`, `url`, `duration`, `bytes`, and `enum_values`.
//! - **Aggregated Diagnostics**: Reports all configuration failures in a single pass without leaking secrets.
//! - **Secret Protection**: Dedicated `Secret<T>` wrapper masking values as `[REDACTED]`.
//! - **Dotenv Editor**: Programmatic editing of `.env` files preserving comments and layout.

pub mod editor;
pub mod env;
pub mod error;
pub mod interpolator;
pub mod loader;
pub mod parser;
pub mod rules;
pub mod secret;
pub mod validator;

pub use editor::{EditorLine, EnvEditor};
pub use env::{Env, contains, current, get, get_raw, init, init_if_unset};
pub use error::{EnvError, ValidationError};
pub use interpolator::EnvInterpolator;
pub use loader::EnvLoader;
pub use parser::{EnvParser, ParsedEntry};
pub use rules::{FieldValidator, Rule};
pub use secret::Secret;
pub use validator::EnvValidator;

/// Essential types, traits, and macros for environment variable handling and validation.
pub mod prelude {
    pub use crate::{
        EditorLine, Env, EnvEditor, EnvError, EnvInterpolator, EnvLoader, EnvParser, EnvValidator,
        FieldValidator, ParsedEntry, Rule, Secret, ValidationError, contains, current, env_schema,
        get, get_raw, init, init_if_unset,
    };
}

/// Declarative macro for defining strongly-typed application environment schemas.
///
/// Generates a struct with typed fields, an automatic validator, and loader methods.
#[macro_export]
macro_rules! env_schema {
    (
        $( #[$meta:meta] )*
        $vis:vis struct $name:ident {
            $(
                $( #[$field_meta:meta] )*
                $field_vis:vis $field:ident : $ty:ty = $rule:expr
            ),* $(,)?
        }
    ) => {
        $( #[$meta] )*
        $vis struct $name {
            $(
                $( #[$field_meta] )*
                $field_vis $field: $ty,
            )*
        }

        impl $name {
            /// Constructs the `EnvValidator` schema for this struct.
            pub fn validator() -> $crate::EnvValidator {
                let mut validator = $crate::EnvValidator::new();
                $(
                    let key_str = stringify!($field).to_ascii_uppercase();
                    validator = validator.rule(key_str, $rule);
                )*
                validator
            }

            /// Discovers cascading files, validates the schema, and returns the populated struct.
            pub fn load() -> Result<Self, $crate::EnvError> {
                Self::load_with_loader(&$crate::Env::loader())
            }

            /// Loads and validates using an explicitly configured `EnvLoader`.
            pub fn load_with_loader(loader: &$crate::EnvLoader) -> Result<Self, $crate::EnvError> {
                let env = Self::validator().load_and_validate(loader)?;
                Self::from_env(&env)
            }

            /// Constructs an instance of this struct from an already validated `Env` container.
            pub fn from_env(env: &$crate::Env) -> Result<Self, $crate::EnvError> {
                Ok(Self {
                    $(
                        $field: {
                            let key_str = stringify!($field).to_ascii_uppercase();
                            let val: &$ty = env.get(&key_str)?;
                            val.clone()
                        },
                    )*
                })
            }
        }
    };
}

impl Env {
    /// Creates a new fluent `EnvValidator` schema builder.
    pub fn rules() -> EnvValidator {
        EnvValidator::new()
    }

    /// Creates a new `EnvLoader` for discovering and reading `.env` files.
    pub fn loader() -> EnvLoader {
        EnvLoader::new()
    }
}
