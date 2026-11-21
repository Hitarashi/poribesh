# `poribesh` Agent Instructions

## Scope

`poribesh` provides robust `.env` file parsing, variable interpolation, cascading file resolution, and boot-time schema
validation for Rust applications. It runs synchronously at application startup to ensure invalid configuration prevents
faulty startup immediately.

## Design Rules

1. **Security & Secret Redaction**:
    - Error messages, logs, and debug outputs must NEVER print secret values in cleartext. Report the variable name and
      the constraint that failed (e.g. `API_KEY: must be at least 32 characters long`), never the actual secret value.
    - `Secret<T>` must always redact its value in `Debug`, `Display`, and `Serialize` implementations.
2. **Comprehensive Validation Aggregation**:
    - Do not halt validation on the first error. Collect all missing, unparseable, or invalid variables into a unified
      error report so users can resolve all configuration issues in one pass.
3. **Cyclic Reference Detection**:
    - Prevent infinite loops during variable interpolation (e.g. `A=${B}`, `B=${A}`) by tracking resolution history and
      limiting expansion depth.
4. **Code Style and Lints**:
    - Follow Google's Rust Style Guide.
    - Do not suppress Clippy warnings inline (`#[allow(...)]` or `#[expect(...)]`). Resolve the warning or restructure
      the code instead.
    - Avoid using leading underscores (`_`) on identifiers, variables, parameters, or struct fields.
    - Code formatting must be verified via `cargo +nightly fmt --check`.
5. **Framework Agnostic**:
    - Do not mention any web frameworks in documentation, codebase comments, identifiers, or error messages.
    - Reference only official standards, RFCs, and standard library conventions.
