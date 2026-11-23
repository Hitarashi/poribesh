//! Environment variable loading, cascading, expansion, and schema validation.
//!
//! # Architecture & Features
//! - **Cascading Discovery**: Deterministic resolution across cascading `.env` files.
//! - **Variable Interpolation**: Expands `${VAR}`, `$VAR`, and fallbacks `${VAR:-default}` with cycle detection.
//! - **Container Secret Resolution**: Pluggable secret resolver seam supporting `file:<path>` protocol.
//! - **Schema Validation**: Strongly-typed rules for strings, numerics, booleans, ports, URLs, durations, and bytes.
//! - **Aggregated Diagnostics**: Reports all configuration failures in a single pass without leaking secrets.
//! - **Secret Protection**: Dedicated `Secret<T>` wrapper masking values as `[REDACTED]`.
//! - **Dotenv Editor**: Programmatic editing of `.env` files preserving comments and layout.

pub mod editor;
pub mod env;
pub mod error;
pub mod interpolator;
pub mod parser;
pub mod rules;
pub mod secret;

pub use editor::{EditorLine, EnvEditor};
pub use env::{Env, EnvBuilder, contains, current, get, get_raw, init, init_if_unset};
pub use error::{EnvError, ValidationError};
pub use interpolator::EnvInterpolator;
pub use parser::EnvParser;
pub use rules::{FieldValidator, Rule};
pub use secret::{FsSecretResolver, MemorySecretResolver, Secret, SecretResolver};

/// Type aliases for backward compatibility.
pub type EnvLoader = EnvBuilder;
pub type EnvValidator = EnvBuilder;

/// Essential types, traits, and macros for environment variable handling and validation.
pub mod prelude {
    pub use crate::{
        EditorLine, Env, EnvBuilder, EnvEditor, EnvError, EnvInterpolator, EnvLoader, EnvParser,
        EnvValidator, FieldValidator, FsSecretResolver, MemorySecretResolver, Rule, Secret,
        SecretResolver, ValidationError, contains, current, env_schema, get, get_raw, init,
        init_if_unset,
    };
}

/// Declarative macro for defining strongly-typed application environment schemas.
///
/// Generates a struct with typed fields, an automatic schema builder, and loader methods.
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
            /// Constructs the `EnvBuilder` schema for this struct.
            pub fn builder() -> $crate::EnvBuilder {
                let mut builder = $crate::Env::builder();
                $(
                    let key_str = stringify!($field).to_ascii_uppercase();
                    builder = builder.rule(key_str, $rule);
                )*
                builder
            }

            /// Constructs the `EnvBuilder` schema (alias for `builder`).
            pub fn validator() -> $crate::EnvBuilder {
                Self::builder()
            }

            /// Discovers cascading files, validates the schema, and returns the populated struct.
            pub fn load() -> Result<Self, $crate::EnvError> {
                let env = Self::builder().load()?;
                Self::from_env(&env)
            }

            /// Loads and validates using an explicitly configured `EnvBuilder`.
            pub fn load_with_builder(builder: &$crate::EnvBuilder) -> Result<Self, $crate::EnvError> {
                let env = Self::builder().load_and_validate(builder)?;
                Self::from_env(&env)
            }

            /// Loads and validates using an explicitly configured `EnvBuilder` or `EnvLoader`.
            pub fn load_with_loader(loader: &$crate::EnvBuilder) -> Result<Self, $crate::EnvError> {
                Self::load_with_builder(loader)
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
