use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CommandEntry {
    pub command: Vec<String>,
}

impl CommandEntry {
    pub fn new(content: &Vec<String>) -> CommandEntry {
        CommandEntry {
            command: content.to_owned(),
        }
    }
    pub fn lines(&self) -> Vec<String> {
        self.command.clone()
    }
    pub fn as_string(&self) -> String {
        self.lines().join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct CommandList {
    pub entries: Vec<CommandEntry>,
    pub file: Option<PathBuf>,
    pub max_size: Option<usize>,
}

impl CommandList {
    pub fn new(file: Option<PathBuf>, max_size: Option<usize>) -> CommandList {
        CommandList {
            entries: Vec::new(),
            max_size,
            file,
        }
    }

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
    pub fn as_strings(&self) -> Vec<String> {
        self.entries.iter().map(|bookmark| bookmark.as_string()).collect()
    }
    pub fn get_at(&self, idx: usize) -> Option<&CommandEntry> {
        self.entries.get(idx)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn remove_at(&mut self, idx: usize) {
        self.entries.remove(idx);
    }
    pub fn remove_entry(&mut self, entry: &CommandEntry) {
        if let Some(idx) = self.entries.iter().position(|e| e == entry) {
            self.entries.remove(idx);
        }
        self.write_to_file();
    }
    pub fn toggle_entry(&mut self, entry: CommandEntry) {
        if !entry.command.is_empty() {
            if self.entries.contains(&entry) {
                self.remove_entry(&entry)
            } else {
                self.push(entry);
            }
        }
    }

    pub fn serialize(&self) -> String {
        self.as_strings().join("\n---\n")
    }
    pub fn deserialize(path: Option<PathBuf>, max_size: Option<usize>, lines: &str) -> CommandList {
        let mut entries = CommandList::new(path, max_size);
        let mut current_entry = Vec::new();
        for line in lines.lines().filter(|x| !x.is_empty()) {
            if line == "---" {
                entries.push(CommandEntry::new(&current_entry));
                current_entry = Vec::new();
            } else {
                current_entry.push(line.to_owned());
            }
        }
        if !current_entry.is_empty() {
            entries.push(CommandEntry::new(&current_entry)); // add last started bookmark
        }

        // remove entries to fit into max_size
        if let Some(max_size) = max_size {
            if entries.len() > max_size {
                entries.entries.drain(0..(entries.len() - max_size));
            }
        }
        entries
    }

    pub fn write_to_file(&self) {
        if let Some(file) = &self.file {
            let mut file = File::create(file).unwrap();
            file.write_all(self.serialize().as_bytes()).unwrap();
        }
    }

    pub fn load_from_file(path: PathBuf, max_size: Option<usize>) -> CommandList {
        if let Some(mut file) = File::open(path.clone()).ok() {
            let mut contents = String::new();
            file.read_to_string(&mut contents).ok();
            CommandList::deserialize(Some(path), max_size, &contents)
        } else {
            CommandList::new(Some(path), max_size)
        }
    }
}
