use std::collections::BTreeMap;
use std::io;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn get_userpath(env: &BTreeMap<String, String>, name: &str, home: &PathBuf) -> Option<PathBuf> {
    env.get(name).map(PathBuf::from).and_then(|mut path| {
        if path.starts_with("$HOME") {
            path = home.join(path.strip_prefix("$HOME").unwrap());
        }

        if path.is_absolute() {
            Some(path)
        } else {
            None
        }
    })
}

pub(crate) fn write_file<P>(home: &PathBuf, path: P) -> io::Result<PathBuf>
where
    P: AsRef<Path>,
{
    match path.as_ref().parent() {
        Some(parent) => fs::create_dir_all(home.join(parent))?,
        None => fs::create_dir_all(home)?,
    }
    Ok(PathBuf::from(home.join(path.as_ref())))
}

pub(crate) fn create_directory<P>(home: &PathBuf, path: P) -> io::Result<PathBuf>
where
    P: AsRef<Path>,
{
    let full_path = home.join(path.as_ref());
    fs::create_dir_all(&full_path)?;
    Ok(full_path)
}

pub(crate) fn path_exists<P: ?Sized + AsRef<Path>>(path: &P) -> bool {
    pub(crate) fn inner(path: &Path) -> bool {
        fs::metadata(path).is_ok()
    }
    inner(path.as_ref())
}

pub(crate) fn read_file(
    home: &PathBuf,
    dirs: &Vec<PathBuf>,
    user_prefix: &Path,
    shared_prefix: &Path,
    path: &Path,
) -> Option<PathBuf> {
    let full_path = home.join(user_prefix).join(path);
    if path_exists(&full_path) {
        return Some(full_path);
    }
    for dir in dirs.iter() {
        let full_path = dir.join(shared_prefix).join(path);
        if path_exists(&full_path) {
            return Some(full_path);
        }
    }
    None
}

use std::vec::IntoIter as VecIter;
pub struct FileFindIterator {
    search_dirs: VecIter<PathBuf>,
    relpath: PathBuf,
}

impl FileFindIterator {
    pub(crate) fn new(
        home: &PathBuf,
        dirs: &Vec<PathBuf>,
        user_prefix: &Path,
        shared_prefix: &Path,
        path: &Path,
    ) -> FileFindIterator {
        let mut search_dirs = Vec::new();
        for dir in dirs.iter().rev() {
            search_dirs.push(dir.join(shared_prefix));
        }
        search_dirs.push(home.join(user_prefix));
        FileFindIterator {
            search_dirs: search_dirs.into_iter(),
            relpath: path.to_path_buf(),
        }
    }
}

impl Iterator for FileFindIterator {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let dir = self.search_dirs.next()?;
            let candidate = dir.join(self.relpath.clone());
            if path_exists(&candidate) {
                return Some(candidate);
            }
        }
    }
}

impl DoubleEndedIterator for FileFindIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            let dir = self.search_dirs.next_back()?;
            let candidate = dir.join(self.relpath.clone());
            if path_exists(&candidate) {
                return Some(candidate);
            }
        }
    }
}

pub(crate) fn list_files(
    home: &Path,
    dirs: &[PathBuf],
    user_prefix: &Path,
    shared_prefix: &Path,
    path: &Path,
) -> Vec<PathBuf> {
    pub(crate) fn read_dir(dir: &Path, into: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            into.extend(
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path()),
            )
        }
    }
    let mut files = Vec::new();
    read_dir(&home.join(user_prefix).join(path), &mut files);
    for dir in dirs {
        read_dir(&dir.join(shared_prefix).join(path), &mut files);
    }
    files
}

pub(crate) fn list_files_once(
    home: &Path,
    dirs: &[PathBuf],
    user_prefix: &Path,
    shared_prefix: &Path,
    path: &Path,
) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    list_files(home, dirs, user_prefix, shared_prefix, path)
        .into_iter()
        .filter(|path| match path.clone().file_name() {
            None => false,
            Some(filename) => {
                if seen.contains(filename) {
                    false
                } else {
                    seen.insert(filename.to_owned());
                    true
                }
            }
        })
        .collect::<Vec<_>>()
}
