use std::{
    fs,
    path::{Path, PathBuf},
};

pub use crate::parser::EditorLine;
use crate::{error::EnvError, parser::parse_ast};

/// Allows programmatic reading, editing, and saving of `.env` files while preserving comments,
/// empty lines, and original formatting.
#[derive(Debug, Clone)]
pub struct EnvEditor {
    file_path: Option<PathBuf>,
    lines: Vec<EditorLine>,
}

impl EnvEditor {
    /// Loads a `.env` file from the specified path for editing.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EnvError> {
        let path_buf = path.as_ref().to_path_buf();
        let contents = fs::read_to_string(&path_buf)?;
        let mut editor = Self::parse(&contents);
        editor.file_path = Some(path_buf);
        Ok(editor)
    }

    /// Loads a `.env` file from the specified path for editing. Alias for `load`.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, EnvError> {
        Self::load(path)
    }

    /// Parses `.env` content from a string for editing.
    pub fn parse(contents: &str) -> Self {
        let lines = parse_ast(contents);
        Self {
            file_path: None,
            lines,
        }
    }

    /// Retrieves the current value of a variable if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        for line in &self.lines {
            if let EditorLine::Entry {
                key: entry_key,
                value,
                ..
            } = line
                && entry_key == key
            {
                return Some(value.as_str());
            }
        }
        None
    }

    /// Checks whether an environment variable exists in the editor.
    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Checks whether an entry has an `export ` prefix.
    pub fn has_export(&self, key: &str) -> bool {
        for line in &self.lines {
            if let EditorLine::Entry {
                key: entry_key,
                has_export,
                ..
            } = line
                && entry_key == key
            {
                return *has_export;
            }
        }
        false
    }

    /// Returns a list of all variable keys present in the editor.
    pub fn keys(&self) -> Vec<&str> {
        let mut result = Vec::new();
        for line in &self.lines {
            if let EditorLine::Entry { key, .. } = line {
                result.push(key.as_str());
            }
        }
        result
    }

    /// Checks whether the editor contains any lines or entries.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
            || self
                .lines
                .iter()
                .all(|line| matches!(line, EditorLine::Empty))
    }

    /// Sets or updates a variable. Preserves line position if already present.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> &mut Self {
        let key_str = key.into();
        let val_str = value.into();
        let mut found = false;

        let needs_quotes = val_str.contains(' ') || val_str.contains('#') || val_str.is_empty();

        for line in &mut self.lines {
            if let EditorLine::Entry {
                key: entry_key,
                value: entry_value,
                quote,
                ..
            } = line
                && entry_key == &key_str
            {
                *entry_value = val_str.clone();
                if quote.is_none() && needs_quotes {
                    *quote = Some('"');
                }
                found = true;
                break;
            }
        }

        if !found {
            let quote = if needs_quotes { Some('"') } else { None };
            self.lines.push(EditorLine::Entry {
                has_export: false,
                key: key_str,
                value: val_str,
                quote,
                inline_comment: None,
            });
        }

        self
    }

    /// Removes a variable by key.
    pub fn unset(&mut self, key: &str) -> &mut Self {
        self.lines.retain(|line| match line {
            EditorLine::Entry { key: entry_key, .. } => entry_key != key,
            _ => true,
        });
        self
    }

    /// Ensures that a variable exists in a companion `.env.example` file (with an empty placeholder value).
    pub fn sync_example(&self, example_path: impl AsRef<Path>, key: &str) -> Result<(), EnvError> {
        let path = example_path.as_ref();
        let mut example_editor = if path.exists() {
            Self::load(path)?
        } else {
            Self {
                file_path: Some(path.to_path_buf()),
                lines: Vec::new(),
            }
        };

        if example_editor.get(key).is_none() {
            example_editor.set(key, "");
            example_editor.save_to(path)?;
        }

        Ok(())
    }

    /// Renders the entire `.env` file back to a string.
    pub fn render(&self) -> String {
        let mut output = String::new();
        for (idx, line) in self.lines.iter().enumerate() {
            output.push_str(&line.render());
            if idx + 1 < self.lines.len() {
                output.push('\n');
            }
        }
        output
    }

    /// Saves the edited contents back to the file it was loaded from.
    pub fn save(&self) -> Result<(), EnvError> {
        match &self.file_path {
            Some(path) => self.save_to(path),
            None => Err(EnvError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no target file path specified for saving",
            ))),
        }
    }

    /// Saves the edited contents to the specified path.
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), EnvError> {
        let content = self.render();
        fs::write(path, content)?;
        Ok(())
    }
}

impl std::str::FromStr for EnvEditor {
    type Err = std::convert::Infallible;

    fn from_str(contents: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(contents))
    }
}
