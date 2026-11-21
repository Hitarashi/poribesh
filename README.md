# Poribesh

Defensive, schema-validated environment variable loader, interpolator, and dotenv file manager for Rust.

[![Crates.io](https://img.shields.io/crates/v/poribesh.svg)](https://crates.io/crates/poribesh)
[![Documentation](https://docs.rs/poribesh/badge.svg)](https://docs.rs/poribesh)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

## Features

- **Cascading Discovery**: Automatically merges `.env.{env}.local` $\to$ `.env.local` $\to$ `.env.{env}` $\to$ `.env`.
- **Variable Interpolation**: Expands `${VAR}`, `$VAR`, and fallbacks `${VAR:-default}` with cycle detection.
- **Container Secret Resolution**: Resolves `file:<path>` protocol values directly from container mounts (e.g. Docker Swarm / Kubernetes).
- **Schema Validation**: Strongly typed rules for `string`, `integer`, `float`, `boolean`, `port`, `url`, `duration`, `bytes`, and `enum_values`.
- **Aggregated Diagnostics**: Gathers all configuration failures into a single actionable error report.
- **Secret Protection**: Dedicated `Secret<T>` wrapper masking values as `[REDACTED]` in `Debug`, `Display`, and `Serialize`.
- **Round-Trip Dotenv Editor**: Safely edit `.env` files while preserving comments, ordering, and blank lines, and sync missing keys to `.env.example`.

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
    let env = Env::rules()
        .rule("HOST", Rule::string().default("127.0.0.1"))
        .rule("PORT", Rule::port().default(3000))
        .rule("NODE_ENV", Rule::string().enum_values(&["development", "production", "test"]))
        .rule("DATABASE_URL", Rule::url())
        .rule("APP_KEY", Rule::string().min(32).secret())
        .validate(Env::loader().load()?)?;

    let port: u16 = env.get("PORT")?;
    let app_key = env.get_secret("APP_KEY")?;
    println!("Server starting on port {port} (key is redacted: {app_key:?})");

    Ok(())
}
```

### Cascading File Loading

```rust
use poribesh::EnvLoader;

// Loads cascading configuration based on active environment (PORIBESH_ENV or APP_ENV)
let values = EnvLoader::new()
    .with_base_dir("./config")
    .load()?;
```

### Variable Interpolation

```env
DOMAIN=example.com
PORT=8080
API_URL=https://${DOMAIN}:${PORT}/v1
TIMEOUT=${REQUEST_TIMEOUT:-30s}
DB_PASSWORD=file:/run/secrets/db_password
```

### Modifying `.env` Files with `EnvEditor`

```rust
use poribesh::EnvEditor;

let mut editor = EnvEditor::from_file(".env")?;
editor.set("DATABASE_URL", "postgres://localhost:5432/app");
editor.sync_example(".env.example")?;
editor.save()?;
```

## Feature Flags

- `regex` *(default)*: Enables regex pattern matching validation rules on strings.
- `serde` *(default)*: Enables deserialization of validated environment stores into strongly typed structs (`env.deserialize::<Config>()`).

## License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
