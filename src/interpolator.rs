use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::error::EnvError;

/// Resolves variable references (`${VAR}`, `$VAR`, `${VAR:-default}`) and custom resolvers.
pub struct EnvInterpolator {
    base_path: PathBuf,
}

impl EnvInterpolator {
    /// Creates a new interpolator with the given base directory for relative path resolution.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Expands all variables in the provided key-value map.
    pub fn interpolate(
        &self,
        map: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, EnvError> {
        let mut resolved = HashMap::with_capacity(map.len());
        let mut stack = Vec::with_capacity(8);

        for key in map.keys() {
            stack.clear();
            self.resolve_variable(key.as_str(), map, &mut stack, &mut resolved)?;
        }

        Ok(resolved)
    }

    fn resolve_variable<'a>(
        &self,
        key: &'a str,
        map: &'a HashMap<String, String>,
        stack: &mut Vec<&'a str>,
        resolved: &mut HashMap<String, String>,
    ) -> Result<(), EnvError> {
        if resolved.contains_key(key) {
            return Ok(());
        }

        if stack.contains(&key) {
            let mut cycle: Vec<String> = stack.iter().map(|s| (*s).to_string()).collect();
            cycle.push(key.to_string());
            return Err(EnvError::CyclicDependency { cycle });
        }

        stack.push(key);

        let expanded = match map.get(key) {
            Some(value) => self.expand_string(value, map, stack, resolved)?,
            None => std::env::var(key).unwrap_or_default(),
        };

        let processed = self.process_resolvers(key, expanded, &self.base_path)?;

        stack.pop();
        resolved.insert(key.to_string(), processed);
        Ok(())
    }

    fn expand_string<'a>(
        &self,
        input: &'a str,
        map: &'a HashMap<String, String>,
        stack: &mut Vec<&'a str>,
        resolved: &mut HashMap<String, String>,
    ) -> Result<String, EnvError> {
        if !input.contains('$') {
            return Ok(input.to_string());
        }

        let bytes = input.as_bytes();
        let len = bytes.len();
        let mut result = String::with_capacity(len);
        let mut index = 0;
        let mut last_copied = 0;

        while index < len {
            if bytes[index] == b'\\' && index + 1 < len && bytes[index + 1] == b'$' {
                result.push_str(&input[last_copied..index]);
                result.push('$');
                index += 2;
                last_copied = index;
                continue;
            }

            if bytes[index] == b'$' {
                if index + 1 < len && bytes[index + 1] == b'{' {
                    let start_brace = index + 2;
                    let mut end_brace = None;
                    let mut depth = 1;
                    let mut scan_index = start_brace;

                    while scan_index < len {
                        if bytes[scan_index] == b'{' {
                            depth += 1;
                        } else if bytes[scan_index] == b'}' {
                            depth -= 1;
                            if depth == 0 {
                                end_brace = Some(scan_index);
                                break;
                            }
                        }
                        scan_index += 1;
                    }

                    if let Some(close_pos) = end_brace {
                        result.push_str(&input[last_copied..index]);
                        let inner = &input[start_brace..close_pos];
                        index = close_pos + 1;
                        last_copied = index;

                        if let Some(separator_pos) = inner.find(":-") {
                            let var_name = inner[..separator_pos].trim();
                            let fallback_raw = &inner[separator_pos + 2..];

                            let value = if stack.contains(&var_name) {
                                std::env::var(var_name).unwrap_or_default()
                            } else {
                                self.resolve_variable(var_name, map, stack, resolved)?;
                                resolved.get(var_name).cloned().unwrap_or_default()
                            };

                            if !value.is_empty() {
                                result.push_str(&value);
                            } else {
                                let fallback_expanded =
                                    self.expand_string(fallback_raw, map, stack, resolved)?;
                                result.push_str(&fallback_expanded);
                            }
                        } else {
                            let var_name = inner.trim();
                            self.resolve_variable(var_name, map, stack, resolved)?;
                            if let Some(val) = resolved.get(var_name) {
                                result.push_str(val);
                            }
                        }
                        continue;
                    }
                } else if index + 1 < len
                    && (bytes[index + 1].is_ascii_alphabetic() || bytes[index + 1] == b'_')
                {
                    result.push_str(&input[last_copied..index]);
                    let start_var = index + 1;
                    let mut end_var = start_var;
                    while end_var < len
                        && (bytes[end_var].is_ascii_alphanumeric() || bytes[end_var] == b'_')
                    {
                        end_var += 1;
                    }

                    let var_name = &input[start_var..end_var];
                    index = end_var;
                    last_copied = index;

                    self.resolve_variable(var_name, map, stack, resolved)?;
                    if let Some(val) = resolved.get(var_name) {
                        result.push_str(val);
                    }
                    continue;
                }
            }

            index += 1;
        }

        if last_copied < len {
            result.push_str(&input[last_copied..]);
        }

        Ok(result)
    }

    fn process_resolvers(
        &self,
        key: &str,
        value: String,
        base_path: &Path,
    ) -> Result<String, EnvError> {
        if let Some(file_path_str) = value.strip_prefix("file:") {
            let relative_or_absolute = Path::new(file_path_str.trim());
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
                    key: key.to_string(),
                    message: format!(
                        "unable to read secret file `{}`: {err}",
                        target_path.display()
                    ),
                }),
            }
        } else {
            Ok(value)
        }
    }
}
