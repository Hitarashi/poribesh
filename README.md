# Poribesh (পরিবেশ)

Defensive, schema-validated environment variable loader, interpolator, and configuration manager for Rust.

[![Crates.io](https://img.shields.io/crates/v/poribesh.svg)](https://crates.io/crates/poribesh)
[![Documentation](https://docs.rs/poribesh/badge.svg)](https://docs.rs/poribesh)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

> **পরিবেশ** (*Poribesh* — Bengali for *environment*) provides boot-time configuration ingestion, variable expansion, cascading resolution, and schema validation with zero process environment mutation.

## Features

- **Deep Configuration Intake**: Load, discover, interpolate, and validate all in a single cohesive call chain via `Env::builder()`.
- **Cascading Discovery**: Deterministically resolves `.env.{env}.local` $\to$ `.env.local` $\to$ `.env.{env}` $\to$ `.env`.
- **Variable Interpolation**: Expands `${VAR}`, `$VAR`, and fallbacks `${VAR:-default}` with cycle detection.
- **Pluggable Secret Resolvers**: Resolves `file:<path>` protocol values directly from container mounts (Docker Swarm / Kubernetes), with an in-memory adapter for hermetic unit testing.
- **Flat Schema Validation**: Fast, zero-allocation rules for `string`, `integer`, `float`, `boolean`, `port`, `url`, `duration`, `bytes`, and `enum_values`.
- **Aggregated Diagnostics**: Gathers all configuration failures into a single actionable error report.
- **Secret Protection**: Dedicated `Secret<T>` wrapper masking values as `[REDACTED]` in `Debug`, `Display`, and `Serialize`.
- **Round-Trip Dotenv Editor**: Programmatically edit `.env` files while preserving comments, ordering, and blank lines, and sync missing keys to `.env.example`.

## Installation

Add `poribesh` to your `Cargo.toml`:

```toml
[dependencies]
poribesh = "0.1.0"
```

## Quick Start

### Basic Loading & Validation

```rust
use poribesh::{Env, Rule};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = Env::builder()
        .rule("HOST", Rule::string().default("127.0.0.1".to_string()))
        .rule("PORT", Rule::port().default(3000))
        .rule("NODE_ENV", Rule::string().enum_values(&["development", "production", "test"]))
        .rule("DATABASE_URL", Rule::url())
        .rule("APP_KEY", Rule::secret(Rule::string().min_length(32)))
        .load()?;

    let port: u16 = env.get("PORT")?;
    let app_key = env.get_secret::<String>("APP_KEY")?;
    println!("Server starting on port {port} (key is redacted: {app_key:?})");

    Ok(())
}
```

### Cascading File Loading

```rust
use poribesh::Env;

// Loads cascading configuration based on active environment (PORIBESH_ENV or APP_ENV)
let env = Env::builder()
    .base_dir("./config")
    .load()?;
```

### Variable Interpolation & Secret Resolvers

```env
DOMAIN=example.com
PORT=8080
API_URL=https://${DOMAIN}:${PORT}/v1
TIMEOUT=${REQUEST_TIMEOUT:-30s}
DB_PASSWORD=file:/run/secrets/db_password
```

For hermetic testing without physical filesystem dependencies:

```rust
use poribesh::{Env, MemorySecretResolver, Rule};

let env = Env::builder()
    .secret_resolver(MemorySecretResolver::new().with_secret("/run/secrets/db_password", "top_secret"))
    .rule("DB_PASSWORD", Rule::string())
    .from_str("DB_PASSWORD=file:/run/secrets/db_password")?;

assert_eq!(env.get::<String>("DB_PASSWORD")?, "top_secret");
```

### Modifying `.env` Files with `EnvEditor`

```rust
use poribesh::EnvEditor;

let mut editor = EnvEditor::from_file(".env")?;
editor.set("DATABASE_URL", "postgres://localhost:5432/app");
editor.sync_example(".env.example", "DATABASE_URL")?;
editor.save()?;
```

## Feature Flags

- `regex` *(default)*: Enables regex pattern matching validation rules on strings.
- `serde` *(default)*: Enables deserialization of validated environment stores into strongly typed structs (`env.deserialize::<Config>()`).
- `jachai`: Opt-in rich schema validation adapter using the `jachai` declarative validation library.

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
