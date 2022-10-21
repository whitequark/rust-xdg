#![cfg(any(unix, target_os = "redox"))]

extern crate dirs;
extern crate dotenv_parser;

use std::fmt;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;

mod error;
mod base;
mod user;
mod util;

pub use error::*;
pub use base::*;
pub use user::*;
use util::*;

#[derive(Copy, Clone)]
struct Permissions(u32);

impl fmt::Debug for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Permissions(p) = *self;
        write!(f, "{:#05o}", p)
    }
}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[test]
fn user_dir_test() {
    let user_dir = UserDirectories::new().expect("");

    println!("{:?}", user_dir);
}

#[cfg(test)]
fn path_is_dir<P: ?Sized + AsRef<Path>>(path: &P) -> bool {
    fn inner(path: &Path) -> bool {
        fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    }
    inner(path.as_ref())
}

#[cfg(test)]
fn make_absolute<P>(path: P) -> PathBuf where P: AsRef<Path> {
    env::current_dir().unwrap().join(path.as_ref())
}

#[cfg(test)]
fn iter_after<A, I, J>(mut iter: I, mut prefix: J) -> Option<I> where
    I: Iterator<Item=A> + Clone, J: Iterator<Item=A>, A: PartialEq
{
    loop {
        let mut iter_next = iter.clone();
        match (iter_next.next(), prefix.next()) {
            (Some(x), Some(y)) => {
                if x != y { return None }
            }
            (Some(_), None) => return Some(iter),
            (None, None) => return Some(iter),
            (None, Some(_)) => return None,
        }
        iter = iter_next;
    }
}

#[cfg(test)]
fn make_relative<P>(path: P) -> PathBuf where P: AsRef<Path> {
    iter_after(path.as_ref().components(), env::current_dir().unwrap().components())
        .unwrap().as_path().to_owned()
}

#[cfg(test)]
fn make_env(vars: Vec<(&'static str, String)>) ->
        Box<dyn Fn(&str)->Option<OsString>> {
    return Box::new(move |name| {
        for &(key, ref value) in vars.iter() {
            if key == name { return Some(OsString::from(value)) }
        }
        None
    })
}

#[test]
fn test_files_exists() {
    assert!(path_exists("test_files"));
    assert!(fs::metadata("test_files/runtime-bad")
                 .unwrap().permissions().mode() & 0o077 != 0);
}

#[test]
fn test_bad_environment() {
    let xd = BaseDirectories::with_env("", "", &*make_env(vec![
            ("HOME", "test_files/user".to_string()),
            ("XDG_DATA_HOME", "test_files/user/data".to_string()),
            ("XDG_CONFIG_HOME", "test_files/user/config".to_string()),
            ("XDG_CACHE_HOME", "test_files/user/cache".to_string()),
            ("XDG_DATA_DIRS", "test_files/user/data".to_string()),
            ("XDG_CONFIG_DIRS", "test_files/user/config".to_string()),
            ("XDG_RUNTIME_DIR", "test_files/runtime-bad".to_string())
        ])).unwrap();
    assert_eq!(xd.find_data_file("everywhere"), None);
    assert_eq!(xd.find_config_file("everywhere"), None);
    assert_eq!(xd.find_cache_file("everywhere"), None);
}

#[test]
fn test_good_environment() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("", "", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
            // ("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
        ])).unwrap();
    assert!(xd.find_data_file("everywhere") != None);
    assert!(xd.find_config_file("everywhere") != None);
    assert!(xd.find_cache_file("everywhere") != None);

    let mut config_files = xd.find_config_files("everywhere");
    assert_eq!(config_files.next(),
        Some(PathBuf::from(format!("{}/test_files/system2/config/everywhere", cwd))));
    assert_eq!(config_files.next(),
        Some(PathBuf::from(format!("{}/test_files/system1/config/everywhere", cwd))));
    assert_eq!(config_files.next(),
        Some(PathBuf::from(format!("{}/test_files/user/config/everywhere", cwd))));
    assert_eq!(config_files.next(), None);

    let mut data_files = xd.find_data_files("everywhere");
    assert_eq!(data_files.next(),
        Some(PathBuf::from(format!("{}/test_files/system2/data/everywhere", cwd))));
    assert_eq!(data_files.next(),
        Some(PathBuf::from(format!("{}/test_files/system1/data/everywhere", cwd))));
    assert_eq!(data_files.next(),
        Some(PathBuf::from(format!("{}/test_files/user/data/everywhere", cwd))));
    assert_eq!(data_files.next(), None);
}

#[test]
fn test_runtime_bad() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("", "", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
        ])).unwrap();
    assert!(xd.has_runtime_directory() == false);
}

#[test]
fn test_runtime_good() {
    use std::fs::File;

    let test_runtime_dir = make_absolute(&"test_files/runtime-good");
    let _ = fs::remove_dir_all(&test_runtime_dir);
    fs::create_dir_all(&test_runtime_dir).unwrap();

    let mut perms = fs::metadata(&test_runtime_dir).unwrap().permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&test_runtime_dir, perms).unwrap();

    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("", "", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-good", cwd)),
        ])).unwrap();

    xd.create_runtime_directory("foo").unwrap();
    assert!(path_is_dir("test_files/runtime-good/foo"));
    let w = xd.place_runtime_file("bar/baz").unwrap();
    assert!(path_is_dir("test_files/runtime-good/bar"));
    assert!(!path_exists("test_files/runtime-good/bar/baz"));
    File::create(&w).unwrap();
    assert!(path_exists("test_files/runtime-good/bar/baz"));
    assert!(xd.find_runtime_file("bar/baz") == Some(w.clone()));
    File::open(&w).unwrap();
    fs::remove_file(&w).unwrap();
    let root = xd.list_runtime_files(".");
    let mut root = root.into_iter().map(|p| make_relative(&p)).collect::<Vec<_>>();
    root.sort();
    assert_eq!(root,
               vec![PathBuf::from("test_files/runtime-good/bar"),
                    PathBuf::from("test_files/runtime-good/foo")]);
    assert!(xd.list_runtime_files("bar").is_empty());
    assert!(xd.find_runtime_file("foo/qux").is_none());
    assert!(xd.find_runtime_file("qux/foo").is_none());
    assert!(!path_exists("test_files/runtime-good/qux"));
}

#[test]
fn test_lists() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("", "", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
        ])).unwrap();

    let files = xd.list_config_files(".");
    let mut files = files.into_iter().map(|p| make_relative(&p)).collect::<Vec<_>>();
    files.sort();
    assert_eq!(files,
        [
            "test_files/system1/config/both_system_config.file",
            "test_files/system1/config/everywhere",
            "test_files/system1/config/myapp",
            "test_files/system1/config/system1_config.file",
            "test_files/system2/config/both_system_config.file",
            "test_files/system2/config/everywhere",
            "test_files/system2/config/system2_config.file",
            "test_files/user/config/everywhere",
            "test_files/user/config/myapp",
            "test_files/user/config/user_config.file",
        ].iter().map(PathBuf::from).collect::<Vec<_>>());

    let files = xd.list_config_files_once(".");
    let mut files = files.into_iter().map(|p| make_relative(&p)).collect::<Vec<_>>();
    files.sort();
    assert_eq!(files,
        [
            "test_files/system1/config/both_system_config.file",
            "test_files/system1/config/system1_config.file",
            "test_files/system2/config/system2_config.file",
            "test_files/user/config/everywhere",
            "test_files/user/config/myapp",
            "test_files/user/config/user_config.file",
        ].iter().map(PathBuf::from).collect::<Vec<_>>());
}

#[test]
fn test_get_file() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("", "", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_RUNTIME_DIR", format!("{}/test_files/user/runtime", cwd)),
        ])).unwrap();

    let path = format!("{}/test_files/user/runtime/", cwd);
    let metadata = fs::metadata(&path).expect("Could not read metadata for runtime directory");
    let mut perms = metadata.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&path, perms).expect("Could not set permissions for runtime directory");

    let file = xd.get_config_file("myapp/user_config.file");
    assert_eq!(file, PathBuf::from(&format!("{}/test_files/user/config/myapp/user_config.file", cwd)));

    let file = xd.get_data_file("user_data.file");
    assert_eq!(file, PathBuf::from(&format!("{}/test_files/user/data/user_data.file", cwd)));

    let file = xd.get_cache_file("user_cache.file");
    assert_eq!(file, PathBuf::from(&format!("{}/test_files/user/cache/user_cache.file", cwd)));

    let file = xd.get_runtime_file("user_runtime.file").unwrap();
    assert_eq!(file, PathBuf::from(&format!("{}/test_files/user/runtime/user_runtime.file", cwd)));
}

#[test]
fn test_prefix() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("myapp", "", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
        ])).unwrap();
    assert_eq!(xd.get_cache_file("cache.db"),
        PathBuf::from(&format!("{}/test_files/user/cache/myapp/cache.db", cwd)));
    assert_eq!(xd.place_cache_file("cache.db").unwrap(),
               PathBuf::from(&format!("{}/test_files/user/cache/myapp/cache.db", cwd)));
}

#[test]
fn test_profile() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("myapp", "default_profile", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system1/config", cwd)),
       ])).unwrap();
    assert_eq!(xd.find_config_file("system1_config.file").unwrap(),
               // Does *not* include default_profile
               PathBuf::from(&format!("{}/test_files/system1/config/myapp/system1_config.file", cwd)));
    assert_eq!(xd.find_config_file("user_config.file").unwrap(),
               // Includes default_profile
               PathBuf::from(&format!("{}/test_files/user/config/myapp/default_profile/user_config.file", cwd)));
}

/// Ensure that entries in XDG_CONFIG_DIRS can be replaced with symlinks.
#[test]
fn test_symlinks() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let symlinks_dir = format!("{}/test_files/symlinks", cwd);
    let config_dir = format!("{}/config", symlinks_dir);
    let myapp_dir = format!("{}/myapp", config_dir);

    assert!(path_exists(&myapp_dir));
    assert!(path_exists(&config_dir));
    assert!(path_exists(&symlinks_dir));

    let xd = BaseDirectories::with_env(
        "myapp", "", &*make_env(vec![
            ("HOME", symlinks_dir),
            ("XDG_CONFIG_HOME", config_dir),
        ])
    ).unwrap();
    assert_eq!(xd.find_config_file("user_config.file").unwrap(),
               PathBuf::from(&format!("{}/user_config.file", myapp_dir)));
}
