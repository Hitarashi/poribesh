# `poribesh` Context

## Purpose

`poribesh` provides environment variable file parsing, variable expansion (interpolation), cascading configuration
loading, and boot-time schema validation for Rust applications.

Unlike raw `.env` loaders that inject untyped strings directly into the process environment, `poribesh` enforces schema
validation so applications cannot start with missing, invalid, or malformed configuration.

## Core Capabilities

1. **`.env` File Parsing**:
    - Key-value definitions with inline comments (`#`), single quotes (literal), double quotes (escaped characters),
      and unquoted values.
    - Multi-line quoted strings (both single and double quoted).
    - Standard escape sequences in double-quoted strings (`\n`, `\r`, `\t`, `\\`, `\"`, `\$`).
    - Optional `export ` prefix.

2. **Variable Expansion & Interpolation**:
    - Recursively expands referenced variables using `${VAR}`, `$VAR`, and fallback `${VAR:-default}` syntax.
    - Resolves file-based secrets via `file:<path>` protocol (e.g. for Docker Swarm and Kubernetes secrets).
    - Detects and prevents infinite recursion or cyclic expansions.

3. **Cascading File Priority**:
    - Loads configuration cascading in priority order:
        1. `.env.{env}.local` (highest priority)
        2. `.env.local` (loaded unless in `test` environment)
        3. `.env.{env}`
        4. `.env` (base file)
    - Determines environment via `PORIBESH_ENV` $\to$ `APP_ENV` $\to$ `"development"`.
    - Respects system environment variables over file values by default (with an opt-in override flag).

4. **Schema Definition & Validation**:
    - Fluent rule builder for defining expected variables, their types, and constraints:
        - `string()` (with min/max length, regex, prefixes, suffixes, enums)
        - `integer()` (with optional range)
        - `float()`
        - `boolean()` (parses `true`, `false`, `1`, `0`, `yes`, `no`)
        - `url()`
        - `port()` (valid 1–65535 range)
        - `duration()` (parses `ms`, `s`, `m`, `h`, `d` into `std::time::Duration`)
        - `bytes()` (parses `b`, `kb`, `mb`, `gb`, `tb` into `u64`)
        - `enum_values(&["local", "staging", "production"])`
        - `list()` (parses comma-separated values into `Vec<T>`)
        - `secret()` (wraps values in `Secret<T>` to prevent accidental leakage)
    - Default values and optional vs required declarations.

5. **Secret Protection**:
    - `Secret<T>` wrapper masking raw values as `[REDACTED]` on `Debug`, `Display`, and `Serialize`.
    - Access to inner value requires explicit `.expose_secret()` or `.into_inner()`.

6. **Round-Trip File Editing**:
    - `EnvEditor` for modifying `.env` files while preserving file layout, comments, and empty lines.
    - Synchronizes missing keys into `.env.example` templates.

7. **Actionable Diagnostics**:
    - Aggregates all validation failures into a structured, human-readable error report listing every missing or invalid
      variable at once, rather than failing on the first error.

## Public API Surface

- `Env`: Immutable typed configuration store (`get::<T>`, `get_raw`, `contains`, `deserialize`, `export_to_process`,
  `rules()`, `loader()`).
- `EnvLoader`: Cascading file loader with environment awareness, custom base directory, and system environment override options.
- `EnvParser`: Low-level `.env` syntax parser with multiline, quote, escape, and inline comment support.
- `EnvEditor`: Comment-preserving round-trip `.env` file editor with `.env.example` key synchronization.
- `EnvValidator`: Schema validator aggregating rule checks across keys.
- `Rule<T>`: Fluent rule builder supporting string constraints, numerics, booleans, ports, URLs, durations, bytes, enums, lists, and secrets.
- `Secret<T>`: Secret wrapper redacting cleartext on `Debug`, `Display`, and `Serialize`.
- `EnvError`: Rich diagnostic error type with aggregated validation failures (`ValidationFailed`).
