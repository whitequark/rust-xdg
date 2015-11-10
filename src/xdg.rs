#![cfg(unix)]
#![cfg_attr(test, feature(path_relative_from))]

use std::iter;
use std::path::{Path, PathBuf};
use std::env;
use std::fs;
use std::io::Result as IoResult;
use std::ffi::OsString;

use std::os::unix::fs::PermissionsExt;

/// BaseDirectories allows to look up paths to configuration, data,
/// cache and runtime files in well-known locations according to
/// the [X Desktop Group Base Directory specification][xdg-basedir].
/// [xdg-basedir]: http://standards.freedesktop.org/basedir-spec/basedir-spec-latest.html
///
/// The Base Directory specification defines four kinds of files:
///
///   * **Configuration files** store the application's settings and
///     are often modified during runtime;
///   * **Data files** store supplementary data, such as graphic assets,
///     precomputed tables, documentation, or architecture-independent
///     source code;
///   * **Cache files** store non-essential, transient data that provides
///     a runtime speedup;
///   * **Runtime files** include filesystem objects such are sockets or
///     named pipes that are used for communication internal to the application.
///     Runtime files must not be accessible to anyone except current user.
///
/// # Examples
///
/// To configure paths for application `myapp`:
///
/// ```
/// extern crate xdg;
/// let xdg_dirs = xdg::BaseDirectories::with_prefix("myapp");
/// ```
///
/// To store configuration:
///
/// ```
/// let config_path = xdg_dirs.place_config_file("config.ini")
///                           .expect("cannot create configuration directory");
/// let mut config_file = try!(File::create(config_path));
/// try!(write!(&mut config_file, "configured = 1"));
/// ```
///
/// The `config.ini` file will appear in the proper location for desktop
/// configuration files, most likely `~/.config/myapp/config.ini`.
/// The leading directories will be automatically created.
///
/// To retrieve supplementary data:
///
/// ```
/// let logo_path = xdg_dirs.find_data_file("logo.png")
///                         .expect("application data not present");
/// let mut logo_file = try!(File::open(logo_path));
/// let mut logo = Vec::new();
/// try!(logo_file.read_to_end(&mut logo));
/// ```
///
/// The `logo.png` will be searched in the proper locations for
/// supplementary data files, most likely `~/.local/share/myapp/logo.png`,
/// then `/usr/local/share/myapp/logo.png` and `/usr/share/myapp/logo.png`.
pub struct BaseDirectories {
    prefix: PathBuf,
    data_home: PathBuf,
    config_home: PathBuf,
    cache_home: PathBuf,
    data_dirs: Vec<PathBuf>,
    config_dirs: Vec<PathBuf>,
    runtime_dir: Option<PathBuf>,
}

impl BaseDirectories
{
    /// Reads the process environment, determines the XDG base directories,
    /// and returns a value that can be used for lookup.
    /// The following environment variables are examined:
    ///
    ///   * `HOME`; if not set: use the same fallback as `std::env::home_dir()`;
    ///     if still not available: panic.
    ///   * `XDG_DATA_HOME`; if not set: assumed to be `$HOME/.local/share`.
    ///   * `XDG_CONFIG_HOME`; if not set: assumed to be `$HOME/.config`.
    ///   * `XDG_CACHE_HOME`; if not set: assumed to be `$HOME/.cache`.
    ///   * `XDG_DATA_DIRS`; if not set: assumed to be `/usr/local/share:/usr/share`.
    ///   * `XDG_CONFIG_DIRS`; if not set: assumed to be `/etc/xdg`.
    ///   * `XDG_RUNTIME_DIR`; if not accessible or permissions are not `0700`: panic.
    ///
    /// As per specification, if an environment variable contains a relative path,
    /// the behavior is the same as if it was not set.
    pub fn new() -> BaseDirectories {
        BaseDirectories::with_env("", &|name| env::var_os(name))
    }

    /// Same as [`new()`](#method.new), but `prefix` is implicitly prepended to
    /// every path that is looked up.
    pub fn with_prefix<P>(prefix: P) -> BaseDirectories where P: AsRef<Path> {
        BaseDirectories::with_env(prefix, &|name| env::var_os(name))
    }

    fn with_env<P, T: ?Sized>(prefix: P, env_var: &T) -> BaseDirectories
            where P: AsRef<Path>, T: Fn(&str) -> Option<OsString> {
        fn abspath(path: OsString) -> Option<PathBuf> {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                Some(path)
            } else {
                None
            }
        }

        fn abspaths(paths: OsString) -> Option<Vec<PathBuf>> {
            let paths = env::split_paths(&paths)
                            .map(PathBuf::from)
                            .filter(|ref path| path.is_absolute())
                            .collect::<Vec<_>>();
            if paths.is_empty() {
                None
            } else {
                Some(paths)
            }
        }

        let home = std::env::home_dir().expect("$HOME must be set");

        let data_home   = env_var("XDG_DATA_HOME")
                              .and_then(abspath)
                              .unwrap_or(home.join(".local/share"));
        let config_home = env_var("XDG_CONFIG_HOME")
                              .and_then(abspath)
                              .unwrap_or(home.join(".config"));
        let cache_home  = env_var("XDG_CACHE_HOME")
                              .and_then(abspath)
                              .unwrap_or(home.join(".cache"));
        let data_dirs   = env_var("XDG_DATA_DIRS")
                              .and_then(abspaths)
                              .unwrap_or(vec![PathBuf::from("/usr/local/share"),
                                              PathBuf::from("/usr/share")]);
        let config_dirs = env_var("XDG_CONFIG_DIRS")
                              .and_then(abspaths)
                              .unwrap_or(vec![PathBuf::from("/etc/xdg")]);
        let runtime_dir = env_var("XDG_RUNTIME_DIR")
                              .and_then(abspath); // optional

        // If XDG_RUNTIME_DIR is in the environment but not secure,
        // do not allow recovery.
        if let Some(ref runtime_dir) = runtime_dir {
            match runtime_dir.metadata() {
                Err(_) => {
                    panic!("$XDG_RUNTIME_DIR must be accessible by the current user");
                }
                Ok(metadata) => {
                    if metadata.permissions().mode() & 0o077 != 0 {
                        panic!("$XDG_RUNTIME_DIR must be secure: have permissions 0700");
                    }
                }
            }
        }

        BaseDirectories {
            prefix: PathBuf::from(prefix.as_ref()),
            data_home: data_home,
            config_home: config_home,
            cache_home: cache_home,
            data_dirs: data_dirs,
            config_dirs: config_dirs,
            runtime_dir: runtime_dir,
        }
    }

    /// Returns `true` if `XDG_RUNTIME_DIR` is available, `false` otherwise.
    pub fn has_runtime_directory(&self) -> bool {
        self.runtime_dir.is_some()
    }

    fn get_runtime_directory(&self) -> &PathBuf {
        self.runtime_dir.as_ref().expect("$XDG_RUNTIME_DIR must be set")
    }

    /// Given a relative path `path`, returns an absolute path in
    /// `XDG_CONFIG_HOME` where a configuration file may be stored.
    /// Leading directories in the returned path are pre-created;
    /// if that is not possible, an error is returned.
    pub fn place_config_file<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        write_file(&self.config_home, self.prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a data file in `XDG_DATA_HOME`.
    pub fn place_data_file<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        write_file(&self.data_home, self.prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a cache file in `XDG_CACHE_HOME`.
    pub fn place_cache_file<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        write_file(&self.cache_home, self.prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a runtime file in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, panics.
    pub fn place_runtime_file<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        write_file(self.get_runtime_directory(), self.prefix.join(path))
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// configuration file, or `None`. Searches `XDG_CONFIG_HOME` and then
    /// `XDG_CONFIG_DIRS`.
    pub fn find_config_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(&self.config_home, &self.config_dirs,
                  self.prefix.join(path))
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// configuration file, or `None`. Searches `XDG_DATA_HOME` and then
    /// `XDG_DATA_DIRS`.
    pub fn find_data_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(&self.data_home, &self.data_dirs,
                  self.prefix.join(path))
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// configuration file, or `None`. Searches `XDG_CACHE_HOME`.
    pub fn find_cache_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(&self.cache_home, &Vec::new(),
                  self.prefix.join(path))
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// runtime file, or `None`. Searches `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, panics.
    pub fn find_runtime_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(self.get_runtime_directory(), &Vec::new(),
                  self.prefix.join(path))
    }

    /// Given a relative path `path`, returns an absolute path to a configuration
    /// directory in `XDG_CONFIG_HOME`. The directory and all directories
    /// leading to it are created if they did not exist;
    /// if that is not possible, an error is returned.
    pub fn create_config_directory<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        create_directory(&self.config_home,
                         self.prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a data directory in `XDG_DATA_HOME`.
    pub fn create_data_directory<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        create_directory(&self.data_home,
                         self.prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a cache directory in `XDG_CACHE_HOME`.
    pub fn create_cache_directory<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        create_directory(&self.cache_home,
                         self.prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a runtime directory in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, panics.
    pub fn create_runtime_directory<P>(&self, path: P) -> IoResult<PathBuf>
            where P: AsRef<Path> {
        create_directory(self.get_runtime_directory(),
                         self.prefix.join(path))
    }

    /// Given a relative path `path`, list absolute paths to all files
    /// in directories with path `path` in `XDG_CONFIG_HOME` and
    /// `XDG_CONFIG_DIRS`.
    pub fn list_config_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(&self.config_home, &self.config_dirs,
                   self.prefix.join(path))
    }

    /// Like [`list_config_files`](#method.list_config_files), but
    /// only the first occurence of every distinct filename is returned.
    pub fn list_config_files_once<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files_once(&self.config_home, &self.config_dirs,
                        self.prefix.join(path))
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_DATA_HOME` and
    /// `XDG_DATA_DIRS`.
    pub fn list_data_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(&self.data_home, &self.data_dirs,
                   self.prefix.join(path))
    }

    /// Like [`list_data_files`](#method.list_data_files), but
    /// only the first occurence of every distinct filename is returned.
    pub fn list_data_files_once<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files_once(&self.data_home, &self.data_dirs,
                        self.prefix.join(path))
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_CACHE_HOME`.
    pub fn list_cache_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(&self.cache_home, &Vec::new(),
                   self.prefix.join(path))
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, panics.
    pub fn list_runtime_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(self.get_runtime_directory(), &Vec::new(),
                   self.prefix.join(path))
    }

    /// Returns the user-specific data directory (set by `XDG_DATA_HOME`).
    pub fn get_data_home(&self) -> PathBuf {
        self.data_home.join(&self.prefix)
    }

    /// Returns the user-specific configuration directory (set by
    /// `XDG_CONFIG_HOME`).
    pub fn get_config_home(&self) -> PathBuf {
        self.config_home.join(&self.prefix)
    }

    /// Returns the user-specific directory for non-essential (cached) data
    /// (set by `XDG_CACHE_HOME`).
    pub fn get_cache_home(&self) -> PathBuf {
        self.cache_home.join(&self.prefix)
    }

    /// Returns a preference ordered (preferred to less preferred) list of
    /// supplementary data directories, ordered by preference (set by
    /// `XDG_DATA_DIRS`).
    pub fn get_data_dirs(&self) -> Vec<PathBuf> {
        self.data_dirs.iter().map(|p| p.join(&self.prefix)).collect()
    }

    /// Returns a preference ordered (preferred to less preferred) list of
    /// supplementary configuration directories (set by `XDG_CONFIG_DIRS`).
    pub fn get_config_dirs(&self) -> Vec<PathBuf> {
        self.config_dirs.iter().map(|p| p.join(&self.prefix)).collect()
    }
}

fn write_file<P>(home: &PathBuf, path: P) -> IoResult<PathBuf>
        where P: AsRef<Path> {
    match path.as_ref().parent() {
        Some(parent) => try!(fs::create_dir_all(home.join(parent))),
        None => try!(fs::create_dir_all(home)),
    }
    Ok(PathBuf::from(home.join(path.as_ref())))
}

fn create_directory<P>(home: &PathBuf, path: P) -> IoResult<PathBuf>
        where P: AsRef<Path> {
    let full_path = home.join(path.as_ref());
    try!(fs::create_dir_all(&full_path));
    Ok(full_path)
}

fn read_file<P>(home: &PathBuf, dirs: &Vec<PathBuf>, path: P) -> Option<PathBuf>
        where P: AsRef<Path> {
    let full_path = home.join(path.as_ref());
    if full_path.exists() {
        return Some(full_path)
    }
    for dir in dirs.iter() {
        let full_path = dir.join(path.as_ref());
        if full_path.exists() {
            return Some(full_path)
        }
    }
    None
}

fn list_files<P>(home: &PathBuf, dirs: &Vec<PathBuf>, path: P) -> Vec<PathBuf>
        where P: AsRef<Path> {
    iter::once(home)
        .chain(dirs.iter())
        .map(|base_dir| {
            fs::read_dir(base_dir.join(path.as_ref()))
               .map(|dir| dir.filter_map(|entry| entry.ok())
                             .map(|entry| entry.path())
                             .collect::<Vec<_>>())
               .unwrap_or(Vec::new())
        })
        .fold(vec![], |mut accum, paths| { accum.extend(paths); accum })
}

fn list_files_once<P>(home: &PathBuf, dirs: &Vec<PathBuf>, path: P) -> Vec<PathBuf>
        where P: AsRef<Path> {
    let mut seen = std::collections::HashSet::new();
    list_files(home, dirs, path).into_iter().filter(|path| {
        match path.clone().file_name() {
            None => false,
            Some(filename) => {
                if seen.contains(filename) {
                    false
                } else {
                    seen.insert(filename.to_owned());
                    true
                }
            }
        }
    }).collect::<Vec<_>>()
}

#[cfg(test)]
fn make_absolute<P>(path: P) -> PathBuf where P: AsRef<Path> {
    env::current_dir().unwrap().join(path.as_ref())
}

#[cfg(test)]
fn make_relative<P>(path: P) -> PathBuf where P: AsRef<Path> {
    path.as_ref().relative_from(&env::current_dir().unwrap()).unwrap().to_owned()
}

#[cfg(test)]
fn make_env(vars: Vec<(&'static str, String)>) ->
        Box<Fn(&str)->Option<OsString>> {
    return Box::new(move |name| {
        for &(key, ref value) in vars.iter() {
            if key == name { return Some(OsString::from(value)) }
        }
        None
    })
}

#[test]
fn test_files_exists() {
    assert!(Path::new("test_files").exists());
    assert!(Path::new("test_files/runtime-bad")
                 .metadata().unwrap().permissions().mode() & 0o077 != 0);
}

#[test]
fn test_bad_environment() {
    let xd = BaseDirectories::with_env("", &*make_env(vec![
            ("HOME", "test_files/user".to_string()),
            ("XDG_DATA_HOME", "test_files/user/data".to_string()),
            ("XDG_CONFIG_HOME", "test_files/user/config".to_string()),
            ("XDG_CACHE_HOME", "test_files/user/cache".to_string()),
            ("XDG_DATA_DIRS", "test_files/user/data".to_string()),
            ("XDG_CONFIG_DIRS", "test_files/user/config".to_string()),
            ("XDG_RUNTIME_DIR", "test_files/runtime-bad".to_string())
        ]));
    assert_eq!(xd.find_data_file("everywhere"), None);
    assert_eq!(xd.find_config_file("everywhere"), None);
    assert_eq!(xd.find_cache_file("everywhere"), None);
}

#[test]
fn test_good_environment() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
            // ("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
        ]));
    assert!(xd.find_data_file("everywhere") != None);
    assert!(xd.find_config_file("everywhere") != None);
    assert!(xd.find_cache_file("everywhere") != None);
}

#[test]
fn test_runtime_bad() {
    std::thread::spawn(move || {
        let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
        let _ = BaseDirectories::with_env("", &*make_env(vec![
                ("HOME", format!("{}/test_files/user", cwd)),
                ("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
            ]));
    }).join().unwrap_err();
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
    let xd = BaseDirectories::with_env("", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-good", cwd)),
        ]));

    xd.create_runtime_directory("foo").unwrap();
    assert!(Path::new("test_files/runtime-good/foo").is_dir());
    let w = xd.place_runtime_file("bar/baz").unwrap();
    assert!(Path::new("test_files/runtime-good/bar").is_dir());
    assert!(!Path::new("test_files/runtime-good/bar/baz").exists());
    File::create(&w).unwrap();
    assert!(Path::new("test_files/runtime-good/bar/baz").exists());
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
    assert!(!Path::new("test_files/runtime-good/qux").exists());
}

#[test]
fn test_lists() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
        ]));

    let files = xd.list_config_files(".");
    let mut files = files.into_iter().map(|p| make_relative(&p)).collect::<Vec<_>>();
    files.sort();
    assert_eq!(files,
        [
            "test_files/system1/config/both_system_config.file",
            "test_files/system1/config/everywhere",
            "test_files/system1/config/system1_config.file",
            "test_files/system2/config/both_system_config.file",
            "test_files/system2/config/everywhere",
            "test_files/system2/config/system2_config.file",
            "test_files/user/config/everywhere",
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
            "test_files/user/config/user_config.file",
        ].iter().map(PathBuf::from).collect::<Vec<_>>());
}

#[test]
fn test_prefix() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = BaseDirectories::with_env("myapp", &*make_env(vec![
            ("HOME", format!("{}/test_files/user", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
        ]));
    assert_eq!(xd.place_cache_file("cache.db").unwrap(),
               PathBuf::from(&format!("{}/test_files/user/cache/myapp/cache.db", cwd)));
}
