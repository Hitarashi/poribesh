use std::collections::HashMap;

use crate::error::EnvError;

/// An individual line in a `.env` file preserving structure and formatting for editing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorLine {
    /// An empty line.
    Empty,
    /// A comment line starting with `#`.
    Comment(String),
    /// A key-value pair definition.
    Entry {
        /// Whether the entry is prefixed with `export `.
        has_export: bool,
        /// The variable name.
        key: String,
        /// The variable value.
        value: String,
        /// Quoting character used (`'` or `"`), if any.
        quote: Option<char>,
        /// Inline comment appended after the value, if any.
        inline_comment: Option<String>,
    },
    /// A raw line that could not be parsed into a standard entry.
    Raw(String),
}

impl EditorLine {
    /// Renders the editor line back to formatted text.
    pub fn render(&self) -> String {
        match self {
            Self::Empty => String::new(),
            Self::Comment(text) => text.clone(),
            Self::Entry {
                has_export,
                key,
                value,
                quote,
                inline_comment,
            } => {
                let mut rendered = String::new();
                if *has_export {
                    rendered.push_str("export ");
                }
                rendered.push_str(key);
                rendered.push('=');

                match quote {
                    Some(quote_char) => {
                        rendered.push(*quote_char);
                        rendered.push_str(value);
                        rendered.push(*quote_char);
                    }
                    None => {
                        rendered.push_str(value);
                    }
                }

                if let Some(comment) = inline_comment {
                    rendered.push(' ');
                    rendered.push_str(comment);
                }

                rendered
            }
            Self::Raw(text) => text.clone(),
        }
    }
}

/// Parses `.env` content into lossless `EditorLine` AST nodes for editing.
pub fn parse_ast(contents: &str) -> Vec<EditorLine> {
    let lines_vec: Vec<&str> = contents.lines().collect();
    let mut lines = Vec::new();
    let mut line_index = 0;

    while line_index < lines_vec.len() {
        let raw_line = lines_vec[line_index];
        let trimmed = raw_line.trim_start();
        if trimmed.is_empty() {
            lines.push(EditorLine::Empty);
            line_index += 1;
            continue;
        }

        if trimmed.starts_with('#') {
            lines.push(EditorLine::Comment(raw_line.to_string()));
            line_index += 1;
            continue;
        }

        let (has_export, without_export) = if let Some(stripped) = trimmed.strip_prefix("export") {
            if stripped.starts_with(char::is_whitespace) {
                (true, stripped.trim_start())
            } else {
                (false, trimmed)
            }
        } else {
            (false, trimmed)
        };

        if let Some(equals_pos) = without_export.find('=') {
            let key = without_export[..equals_pos].trim().to_string();
            let rest = without_export[equals_pos + 1..].trim_start();

            let (value, quote, inline_comment, next_line_index) =
                parse_editor_value(rest, &lines_vec, line_index);

            lines.push(EditorLine::Entry {
                has_export,
                key,
                value,
                quote,
                inline_comment,
            });
            line_index = next_line_index + 1;
        } else {
            lines.push(EditorLine::Raw(raw_line.to_string()));
            line_index += 1;
        }
    }

    lines
}

fn parse_editor_value(
    rest: &str,
    lines: &[&str],
    current_line_index: usize,
) -> (String, Option<char>, Option<String>, usize) {
    if rest.is_empty() {
        return (String::new(), None, None, current_line_index);
    }

    let first = rest.chars().next().unwrap();
    if first == '\'' || first == '"' {
        let quote_char = first;
        if let Some(close_pos) = rest[1..].find(quote_char) {
            let value = rest[1..1 + close_pos].to_string();
            let remaining = rest[1 + close_pos + 1..].trim_start();
            let inline_comment = if remaining.starts_with('#') {
                Some(remaining.to_string())
            } else {
                None
            };
            return (value, Some(quote_char), inline_comment, current_line_index);
        }

        let mut accumulated = rest[1..].to_string();
        let mut scan_index = current_line_index + 1;
        let mut found_close = false;
        let mut inline_comment = None;

        while scan_index < lines.len() {
            let line_str = lines[scan_index];
            if let Some(close_pos) = line_str.find(quote_char) {
                accumulated.push('\n');
                accumulated.push_str(&line_str[..close_pos]);
                let remaining = line_str[close_pos + 1..].trim_start();
                if remaining.starts_with('#') {
                    inline_comment = Some(remaining.to_string());
                }
                found_close = true;
                break;
            }
            accumulated.push('\n');
            accumulated.push_str(line_str);
            scan_index += 1;
        }

        if found_close {
            return (accumulated, Some(quote_char), inline_comment, scan_index);
        }
    }

    let mut value = String::new();
    let mut inline_comment = None;
    let chars: Vec<char> = rest.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '#' && (index == 0 || chars[index - 1].is_whitespace()) {
            let comment_slice: String = chars[index..].iter().collect();
            inline_comment = Some(comment_slice);
            break;
        }
        value.push(chars[index]);
        index += 1;
    }

    (
        value.trim().to_string(),
        None,
        inline_comment,
        current_line_index,
    )
}

/// Parses `.env` content into key-value entries with escape processing and validation.
pub fn parse_entries(input: &str) -> Result<Vec<(String, String)>, EnvError> {
    let lines: Vec<&str> = input.lines().collect();
    let mut entries = Vec::new();
    let mut line_index = 0;

    while line_index < lines.len() {
        let current_line = lines[line_index];
        let trimmed = current_line.trim_start();
        let entry_line_number = line_index + 1;

        if trimmed.is_empty() || trimmed.starts_with('#') {
            line_index += 1;
            continue;
        }

        let without_export = if let Some(stripped) = trimmed.strip_prefix("export") {
            if stripped.starts_with(char::is_whitespace) {
                stripped.trim_start()
            } else {
                trimmed
            }
        } else {
            trimmed
        };

        let equals_pos = match without_export.find('=') {
            Some(pos) => pos,
            None => {
                return Err(EnvError::Parse {
                    line: entry_line_number,
                    message: format!("missing '=' assignment separator in line: `{current_line}`"),
                });
            }
        };

        let raw_key = without_export[..equals_pos].trim();
        if raw_key.is_empty() {
            return Err(EnvError::Parse {
                line: entry_line_number,
                message: "variable name cannot be empty".to_string(),
            });
        }

        let rest_after_equals = without_export[equals_pos + 1..].trim_start();

        let (value, next_line_index) =
            parse_entry_value(rest_after_equals, &lines, line_index, entry_line_number)?;

        entries.push((raw_key.to_string(), value));
        line_index = next_line_index + 1;
    }

    Ok(entries)
}

/// Parses `.env` content directly into a HashMap.
pub fn parse_to_map(input: &str) -> Result<HashMap<String, String>, EnvError> {
    let entries = parse_entries(input)?;
    let mut map = HashMap::with_capacity(entries.len());
    for (key, value) in entries {
        map.insert(key, value);
    }
    Ok(map)
}

/// Utility parser providing direct parsing functions.
pub struct EnvParser;

impl EnvParser {
    /// Parses raw `.env` string contents into key-value entries.
    pub fn parse(input: &str) -> Result<Vec<(String, String)>, EnvError> {
        parse_entries(input)
    }

    /// Parses `.env` content directly into a HashMap.
    pub fn parse_to_map(input: &str) -> Result<HashMap<String, String>, EnvError> {
        parse_to_map(input)
    }
}

fn parse_entry_value(
    initial_value_slice: &str,
    lines: &[&str],
    current_line_index: usize,
    entry_line_number: usize,
) -> Result<(String, usize), EnvError> {
    if initial_value_slice.is_empty() {
        return Ok((String::new(), current_line_index));
    }

    let first_char = initial_value_slice.chars().next().unwrap();

    if first_char == '\'' {
        parse_single_quoted(
            &initial_value_slice[1..],
            lines,
            current_line_index,
            entry_line_number,
        )
    } else if first_char == '"' {
        parse_double_quoted(
            &initial_value_slice[1..],
            lines,
            current_line_index,
            entry_line_number,
        )
    } else {
        let unquoted = parse_unquoted(initial_value_slice);
        Ok((unquoted, current_line_index))
    }
}

fn parse_single_quoted<'a>(
    mut slice: &'a str,
    lines: &[&'a str],
    mut line_index: usize,
    entry_line_number: usize,
) -> Result<(String, usize), EnvError> {
    let mut accumulated = String::new();

    loop {
        if let Some(closing_pos) = slice.find('\'') {
            accumulated.push_str(&slice[..closing_pos]);
            return Ok((accumulated, line_index));
        }

        accumulated.push_str(slice);
        accumulated.push('\n');

        line_index += 1;
        if line_index >= lines.len() {
            return Err(EnvError::Parse {
                line: entry_line_number,
                message: "unterminated single-quoted string".to_string(),
            });
        }

        slice = lines[line_index];
    }
}

fn parse_double_quoted<'a>(
    mut slice: &'a str,
    lines: &[&'a str],
    mut line_index: usize,
    entry_line_number: usize,
) -> Result<(String, usize), EnvError> {
    if !slice.contains('\\') {
        if let Some(closing_pos) = slice.find('"') {
            return Ok((slice[..closing_pos].to_string(), line_index));
        }
    }

    let mut accumulated = String::new();
    let mut in_escape = false;

    loop {
        let chars = slice.chars();
        for ch in chars {
            if in_escape {
                match ch {
                    'n' => accumulated.push('\n'),
                    'r' => accumulated.push('\r'),
                    't' => accumulated.push('\t'),
                    '\\' => accumulated.push('\\'),
                    '"' => accumulated.push('"'),
                    '$' => accumulated.push_str("\\$"),
                    other => {
                        accumulated.push('\\');
                        accumulated.push(other);
                    }
                }
                in_escape = false;
            } else if ch == '\\' {
                in_escape = true;
            } else if ch == '"' {
                return Ok((accumulated, line_index));
            } else {
                accumulated.push(ch);
            }
        }

        accumulated.push('\n');
        line_index += 1;
        if line_index >= lines.len() {
            return Err(EnvError::Parse {
                line: entry_line_number,
                message: "unterminated double-quoted string".to_string(),
            });
        }

        slice = lines[line_index];
    }
}

fn parse_unquoted(slice: &str) -> String {
    let bytes = slice.as_bytes();
    let mut end = bytes.len();
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'#' && (index == 0 || bytes[index - 1].is_ascii_whitespace()) {
            end = index;
            break;
        }
    }
    slice[..end].trim().to_string()
}
