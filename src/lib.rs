#![cfg(any(unix, target_os = "redox"))]
#![warn(rust_2018_idioms, redundant_semicolons, rust_2024_compatibility)]

mod base_directories;
pub use crate::base_directories::{
    BaseDirectories, Error as BaseDirectoriesError, FileFindIterator,
};
