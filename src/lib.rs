#![cfg(any(unix, target_os = "redox"))]

extern crate dirs;
#[cfg(feature = "serde")]
extern crate serde;

mod base_directories;
pub use base_directories::{BaseDirectories, Error as BaseDirectoriesError, FileFindIterator};
