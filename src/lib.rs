#![cfg(any(unix, target_os = "redox"))]
#![warn(rust_2018_idioms, rust_2021_compatibility)]

mod base_directories;
pub use crate::base_directories::{
    BaseDirectories, Error as BaseDirectoriesError, FileFindIterator,
};
