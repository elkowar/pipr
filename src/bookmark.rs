use std::fs::{DirBuilder, File};
use std::io::prelude::*;
use std::io::{self, BufRead};
use std::path::Path;

const bookmarks_path: &'static str = "~/.config/pipr/bookmarks";

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Bookmark {
    pub content: Vec<String>,
}

impl Bookmark {
    pub fn new(content: Vec<String>) -> Bookmark { Bookmark { content } }
    pub fn to_string(&self) -> String { self.content.join("") }
    pub fn from_string(string: &str) -> Bookmark {
        Bookmark {
            content: string.chars().map(|c| c.to_string()).collect(),
        }
    }
}

pub struct BookmarkList(Vec<Bookmark>);

impl BookmarkList {
    pub fn new() -> BookmarkList { BookmarkList(Vec::new()) }
    pub fn add_bookmark(&mut self, bookmark: Bookmark) {
        self.0.push(bookmark);
        write_to_file(self);
    }
    pub fn as_strings(&self) -> Vec<String> { self.0.iter().map(|bookmark| bookmark.to_string()).collect() }
    pub fn bookmark_at(&self, idx: usize) -> Option<&Bookmark> { self.0.get(idx) }
    pub fn remove_at(&mut self, idx: usize) {
        self.0.remove(idx);
        write_to_file(self);
    }
    pub fn len(&self) -> usize { self.0.len() }
    pub fn toggle_bookmark(&mut self, bookmark: Bookmark) {
        if !bookmark.content.is_empty() {
            if self.0.contains(&bookmark) {
                self.0.remove_item(&bookmark);
            } else {
                self.0.push(bookmark);
            }
        }
    }
}

pub fn load_file() -> Option<BookmarkList> {
    let file = File::open(bookmarks_path).ok()?;
    let mut list = BookmarkList::new();
    for line in io::BufReader::new(file).lines() {
        if let Some(line) = line.ok() {
            list.add_bookmark(Bookmark::from_string(&line));
        }
    }
    Some(list)
}

pub fn write_to_file(bookmarks: &BookmarkList) {
    let path = Path::new(bookmarks_path);
    DirBuilder::new().recursive(true).create(&path.parent().unwrap()).unwrap();
    let mut file = File::create(&path).unwrap();
    file.write_all(bookmarks.as_strings().join("\n").as_bytes()).unwrap();
}
