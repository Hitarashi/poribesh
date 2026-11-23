# poribesh (পরিবেশ) Domain Glossary

Defensive, schema-validated environment variable loader, interpolator, and configuration manager for Rust applications.

## Language

**Configuration**:
The immutable, validated, strongly-typed key-value settings used to bootstrap an application.
_Avoid_: Environment, settings, config object, options

**Rule**:
A declarative constraint and conversion specification applied to a configuration key at startup.
_Avoid_: Validator, field schema, check

**Secret**:
A sensitive configuration value masked from cleartext display, debug printing, and serialization.
_Avoid_: Password, credential, private key, token

**Cascading Discovery**:
The deterministic loading and merging of configuration files in priority order based on the active environment name.
_Avoid_: Config waterfall, env inheritance, file stacking

**Variable Interpolation**:
The recursive expansion of `${VAR}`, `$VAR`, and fallback `${VAR:-default}` syntax across configuration values with cycle detection.
_Avoid_: Variable substitution, template expansion, variable injection

**Secret Resolver**:
An adapter that retrieves sensitive values from external sources (such as mounted container files) using designated URI schemes.
_Avoid_: Secret loader, credential fetcher, file reader
