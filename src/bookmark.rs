use itertools::Itertools;
use std::env;
use std::fs::{DirBuilder, File};
use std::io::prelude::*;
use std::path::Path;
const BOOKMARKS_PATH_RELATIVE_TO_HOME: &'static str = ".config/pipr/bookmarks";

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Bookmark {
    pub content: Vec<String>,
}

impl Bookmark {
    pub fn new(content: &Vec<String>) -> Bookmark {
        Bookmark {
            content: content.to_owned(),
        }
    }
    pub fn to_lines(&self) -> Vec<String> {
        self.content.clone()
    }
    pub fn first_line(&self) -> &str {
        &self.content[0]
    }
}

pub struct BookmarkList(Vec<Bookmark>);

impl BookmarkList {
    pub fn new() -> BookmarkList {
        BookmarkList(Vec::new())
    }

    pub fn add_bookmark(&mut self, bookmark: Bookmark) {
        self.0.push(bookmark);
        write_to_file(self);
    }
    pub fn as_strings(&self) -> Vec<String> {
        self.0.iter().map(|bookmark| bookmark.to_lines().join("\n")).collect()
    }
    pub fn bookmark_at(&self, idx: usize) -> Option<&Bookmark> {
        self.0.get(idx)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn remove_bookmark(&mut self, bookmark: &Bookmark) {
        self.0.remove_item(&bookmark);
        write_to_file(self);
    }
    pub fn toggle_bookmark(&mut self, bookmark: Bookmark) {
        if !bookmark.content.is_empty() {
            if self.0.contains(&bookmark) {
                self.remove_bookmark(&bookmark)
            } else {
                self.add_bookmark(bookmark);
            }
        }
    }
}

impl std::iter::FromIterator<Bookmark> for BookmarkList {
    fn from_iter<T: IntoIterator<Item = Bookmark>>(iter: T) -> Self {
        let mut list = BookmarkList::new();
        for bookmark in iter {
            list.add_bookmark(bookmark);
        }
        list
    }
}

pub fn load_file() -> Option<BookmarkList> {
    let home_path = env::var("HOME").ok()?;
    let bookmarks_path = Path::new(&home_path).join(BOOKMARKS_PATH_RELATIVE_TO_HOME);
    let mut file = File::open(bookmarks_path).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;

    let mut bookmarks = BookmarkList::new();
    let mut current_bookmark = Vec::new();
    for line in contents.lines() {
        if line == "---" {
            bookmarks.add_bookmark(Bookmark::new(&current_bookmark));
            current_bookmark = Vec::new();
        } else {
            current_bookmark.push(line.to_owned());
        }
    }
    bookmarks.add_bookmark(Bookmark::new(&current_bookmark)); // add last started bookmark
    Some(bookmarks)
}

pub fn write_to_file(bookmarks: &BookmarkList) {
    let home_path = env::var("HOME").unwrap();
    let bookmarks_path = Path::new(&home_path).join(BOOKMARKS_PATH_RELATIVE_TO_HOME);
    DirBuilder::new()
        .recursive(true)
        .create(&bookmarks_path.parent().unwrap())
        .unwrap();
    let mut file = File::create(&bookmarks_path).unwrap();

    file.write_all(bookmarks.as_strings().join("\n---\n").as_bytes()).unwrap();
}
