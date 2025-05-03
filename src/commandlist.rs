//! [`CommandList`] is a list of stored commands that can be persisted to disk.
//! This is used, amongst other things, to store bookmarks and the command history.

use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

const SERIALIZATION_ENTRY_SEPERATOR: &str = "---";

/// A command entry consisting of multiple lines of text.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CommandEntry(Vec<String>);

impl CommandEntry {
    /// Creates a new command entry from lines of content.
    pub fn new(content: Vec<String>) -> CommandEntry {
        CommandEntry(content)
    }
    /// Returns the lines in this entry.
    pub fn lines(&self) -> &Vec<String> {
        &self.0
    }
    /// Converts the entry to a single string, joining lines with newlines.
    pub fn as_string(&self) -> String {
        self.lines().join("\n")
    }
}

/// A list of command entries that can be persisted to disk.
/// 
/// When serialized, entries are separated by "---" surrounded by newlines:
/// ```text
/// echo hello
/// ---
/// grep pattern file.txt
/// ---
/// ls -la
/// ```
#[derive(Debug, Clone)]
pub struct CommandList {
    entries: Vec<CommandEntry>,
    file: Option<PathBuf>,
    max_size: Option<usize>,
}

impl CommandList {
    /// Creates a new command list with optional path and size limit.
    pub fn new(file: Option<PathBuf>, max_size: Option<usize>) -> CommandList {
        CommandList {
            entries: Vec::new(),
            max_size,
            file,
        }
    }

    /// Returns all entries in the list.
    pub fn entries(&self) -> &Vec<CommandEntry> {
        &self.entries
    }

    /// Replaces all entries and saves to disk.
    pub fn set_entries(&mut self, entries: Vec<CommandEntry>) {
        self.entries = entries;
        self.write_to_file();
    }

    /// Adds a command entry if not empty or duplicate, respecting max size.
    pub fn push(&mut self, command: CommandEntry) {
        if !command.as_string().is_empty() && self.entries.last() != Some(&command) {
            self.entries.push(command);
            if let Some(max_size) = self.max_size {
                if self.len() > max_size {
                    self.entries.remove(0);
                }
            }
            self.write_to_file();
        }
    }
    /// Returns all entries as strings.
    pub fn as_strings(&self) -> Vec<String> {
        self.entries.iter().map(|x| x.as_string()).collect()
    }

    /// Returns the entry at the given index.
    pub fn get_at(&self, idx: usize) -> Option<&CommandEntry> {
        self.entries.get(idx)
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Removes the given entry if present.
    pub fn remove_entry(&mut self, entry: &CommandEntry) {
        if let Some(idx) = self.entries.iter().position(|e| e == entry) {
            self.entries.remove(idx);
        }
        self.write_to_file();
    }

    /// Adds the entry if not present, or removes it if present.
    pub fn toggle_entry(&mut self, entry: CommandEntry) {
        if !entry.lines().is_empty() {
            if self.entries.contains(&entry) {
                self.remove_entry(&entry)
            } else {
                self.push(entry);
            }
        }
    }

    /// Serializes entries to a string with separators.
    pub fn serialize(&self) -> String {
        self.as_strings().join(&format!("\n{}\n", SERIALIZATION_ENTRY_SEPERATOR))
    }

    /// Creates a [`CommandList`] from serialized string data.
    pub fn deserialize(path: Option<PathBuf>, max_size: Option<usize>, lines: &str) -> CommandList {
        let mut entries = CommandList::new(path, max_size);
        let mut current_entry = Vec::new();
        for line in lines.lines().filter(|x| !x.is_empty()) {
            if line == SERIALIZATION_ENTRY_SEPERATOR {
                entries.push(CommandEntry::new(current_entry));
                current_entry = Vec::new();
            } else {
                current_entry.push(line.to_owned());
            }
        }
        if !current_entry.is_empty() {
            entries.push(CommandEntry::new(current_entry)); // add last started entry
        }

        // remove entries to fit into max_size
        if let Some(max_size) = max_size {
            if entries.len() > max_size {
                entries.entries.drain(0..(entries.len() - max_size));
            }
        }
        entries
    }

    /// Writes entries to file if path is set.
    pub fn write_to_file(&self) {
        if let Some(file) = &self.file {
            let mut file = File::create(file).unwrap();
            file.write_all(self.serialize().as_bytes()).unwrap();
        }
    }

    /// Loads a [`CommandList`] from a file or creates a new one if file doesn't exist.
    pub fn load_from_file(path: PathBuf, max_size: Option<usize>) -> CommandList {
        if let Ok(mut file) = File::open(path.clone()) {
            let mut contents = String::new();
            file.read_to_string(&mut contents).ok();
            CommandList::deserialize(Some(path), max_size, &contents)
        } else {
            CommandList::new(Some(path), max_size)
        }
    }
}
