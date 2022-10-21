use std::io;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::ffi::OsString;

use std::os::unix::fs::PermissionsExt;

use crate::Permissions;
use crate::error::XdgError;
use crate::error::XdgErrorKind::*;

use crate::util::*;

/// BaseDirectories allows to look up paths to configuration, data,
/// cache and runtime files in well-known locations according to
/// the [X Desktop Group Base Directory specification][xdg-basedir].
/// [xdg-basedir]: http://standards.freedesktop.org/basedir-spec/basedir-spec-latest.html
///
/// The Base Directory specification defines five kinds of files:
///
///   * **Configuration files** store the application's settings and
///     are often modified during runtime;
///   * **Data files** store supplementary data, such as graphic assets,
///     precomputed tables, documentation, or architecture-independent
///     source code;
///   * **Cache files** store non-essential, transient data that provides
///     a runtime speedup;
///   * **State files** store logs, history, recently used files and application
///     state (window size, open files, unsaved changes, …);
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
/// let xdg_dirs = xdg::BaseDirectories::with_prefix("myapp").unwrap();
/// ```
///
/// To store configuration:
///
/// ```
/// let config_path = xdg_dirs.place_config_file("config.ini")
///                           .expect("cannot create configuration directory");
/// let mut config_file = File::create(config_path)?;
/// write!(&mut config_file, "configured = 1")?;
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
/// let mut logo_file = File::open(logo_path)?;
/// let mut logo = Vec::new();
/// logo_file.read_to_end(&mut logo)?;
/// ```
///
/// The `logo.png` will be searched in the proper locations for
/// supplementary data files, most likely `~/.local/share/myapp/logo.png`,
/// then `/usr/local/share/myapp/logo.png` and `/usr/share/myapp/logo.png`.
#[derive(Debug, Clone)]
pub struct BaseDirectories {
    shared_prefix: PathBuf,
    user_prefix: PathBuf,
    data_home: PathBuf,
    config_home: PathBuf,
    cache_home: PathBuf,
    state_home: PathBuf,
    data_dirs: Vec<PathBuf>,
    config_dirs: Vec<PathBuf>,
    runtime_dir: Option<PathBuf>,
}

impl BaseDirectories {
    /// Reads the process environment, determines the XDG base directories,
    /// and returns a value that can be used for lookup.
    /// The following environment variables are examined:
    ///
    ///   * `HOME`; if not set: use the same fallback as `dirs::home_dir()`;
    ///     if still not available: return an error.
    ///   * `XDG_DATA_HOME`; if not set: assumed to be `$HOME/.local/share`.
    ///   * `XDG_CONFIG_HOME`; if not set: assumed to be `$HOME/.config`.
    ///   * `XDG_CACHE_HOME`; if not set: assumed to be `$HOME/.cache`.
    ///   * `XDG_STATE_HOME`; if not set: assumed to be `$HOME/.local/state`.
    ///   * `XDG_DATA_DIRS`; if not set: assumed to be `/usr/local/share:/usr/share`.
    ///   * `XDG_CONFIG_DIRS`; if not set: assumed to be `/etc/xdg`.
    ///   * `XDG_RUNTIME_DIR`; if not accessible or permissions are not `0700`:
    ///     record as inaccessible (can be queried with
    ///     [has_runtime_directory](method.has_runtime_directory)).
    ///
    /// As per specification, if an environment variable contains a relative path,
    /// the behavior is the same as if it was not set.
    pub fn new() -> Result<BaseDirectories, XdgError> {
        BaseDirectories::with_env("", "", &|name| env::var_os(name))
    }

    /// Same as [`new()`](#method.new), but `prefix` is implicitly prepended to
    /// every path that is looked up.
    pub fn with_prefix<P>(prefix: P) -> Result<BaseDirectories, XdgError>
            where P: AsRef<Path> {
        BaseDirectories::with_env(prefix, "", &|name| env::var_os(name))
    }

    /// Same as [`with_prefix()`](#method.with_prefix),
    /// with `profile` also implicitly prepended to every path that is looked up,
    /// but only for user-specific directories.
    ///
    /// This allows each user to have mutliple "profiles" with different user-specific data.
    ///
    /// For example:
    ///
    /// ```rust
    /// let dirs = BaseDirectories::with_profile("program-name", "profile-name")
    ///                            .unwrap();
    /// dirs.find_data_file("bar.jpg");
    /// dirs.find_config_file("foo.conf");
    /// ```
    ///
    /// will find `/usr/share/program-name/bar.jpg` (without `profile-name`)
    /// and `~/.config/program-name/profile-name/foo.conf`.
    pub fn with_profile<P1, P2>(prefix: P1, profile: P2)
            -> Result<BaseDirectories, XdgError>
            where P1: AsRef<Path>, P2: AsRef<Path> {
        BaseDirectories::with_env(prefix, profile, &|name| env::var_os(name))
    }

    pub(crate) fn with_env<P1, P2, T: ?Sized>(prefix: P1, profile: P2, env_var: &T)
            -> Result<BaseDirectories, XdgError>
            where P1: AsRef<Path>, P2: AsRef<Path>, T: Fn(&str) -> Option<OsString> {
        BaseDirectories::with_env_impl(prefix.as_ref(), profile.as_ref(), env_var)
    }

    fn with_env_impl<T: ?Sized>(prefix: &Path, profile: &Path, env_var: &T)
            -> Result<BaseDirectories, XdgError>
            where T: Fn(&str) -> Option<OsString> {
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

        let home = dirs::home_dir().ok_or(XdgError::new(HomeMissing))?;

        let data_home   = env_var("XDG_DATA_HOME")
                              .and_then(abspath)
                              .unwrap_or(home.join(".local/share"));
        let config_home = env_var("XDG_CONFIG_HOME")
                              .and_then(abspath)
                              .unwrap_or(home.join(".config"));
        let cache_home  = env_var("XDG_CACHE_HOME")
                              .and_then(abspath)
                              .unwrap_or(home.join(".cache"));
        let state_home  = env_var("XDG_STATE_HOME")
                              .and_then(abspath)
                              .unwrap_or(home.join(".local/state"));
        let data_dirs   = env_var("XDG_DATA_DIRS")
                              .and_then(abspaths)
                              .unwrap_or(vec![PathBuf::from("/usr/local/share"),
                                              PathBuf::from("/usr/share")]);
        let config_dirs = env_var("XDG_CONFIG_DIRS")
                              .and_then(abspaths)
                              .unwrap_or(vec![PathBuf::from("/etc/xdg")]);
        let runtime_dir = env_var("XDG_RUNTIME_DIR")
                              .and_then(abspath); // optional

        let prefix = PathBuf::from(prefix);
        Ok(BaseDirectories {
            user_prefix: prefix.join(profile),
            shared_prefix: prefix,
            data_home,
            config_home,
            cache_home,
            state_home,
            data_dirs,
            config_dirs,
            runtime_dir,
        })
    }

    /// Returns the user-specific runtime directory (set by `XDG_RUNTIME_DIR`).
    pub fn get_runtime_directory(&self) -> Result<&PathBuf, XdgError> {
        if let Some(ref runtime_dir) = self.runtime_dir {
            // If XDG_RUNTIME_DIR is in the environment but not secure,
            // do not allow recovery.
            fs::read_dir(runtime_dir).map_err(|e| {
                XdgError::new(XdgRuntimeDirInaccessible(runtime_dir.clone(), e))
            })?;
            let permissions = fs::metadata(runtime_dir).map_err(|e| {
                XdgError::new(XdgRuntimeDirInaccessible(runtime_dir.clone(), e))
            })?.permissions().mode() as u32;
            if permissions & 0o077 != 0 {
                Err(XdgError::new(XdgRuntimeDirInsecure(runtime_dir.clone(),
                                                     Permissions(permissions))))
            } else {
                Ok(&runtime_dir)
            }
        } else {
            Err(XdgError::new(XdgRuntimeDirMissing))
        }
    }

    /// Returns `true` if `XDG_RUNTIME_DIR` is available, `false` otherwise.
    pub fn has_runtime_directory(&self) -> bool {
        match self.get_runtime_directory() {
            Ok(_) => true,
            _ => false
        }
    }

    /// Like [`place_config_file()`](#method.place_config_file), but does
    /// not create any directories.
    pub fn get_config_file<P>(&self, path: P) -> PathBuf
            where P: AsRef<Path> {
        self.config_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_data_file()`](#method.place_data_file), but does
    /// not create any directories.
    pub fn get_data_file<P>(&self, path: P) -> PathBuf
            where P: AsRef<Path> {
        self.data_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_cache_file()`](#method.place_cache_file), but does
    /// not create any directories.
    pub fn get_cache_file<P>(&self, path: P) -> PathBuf
            where P: AsRef<Path> {
        self.cache_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_state_file()`](#method.place_state_file), but does
    /// not create any directories.
    pub fn get_state_file<P>(&self, path: P) -> PathBuf
            where P: AsRef<Path> {
        self.state_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_runtime_file()`](#method.place_runtime_file), but does
    /// not create any directories.
    /// If `XDG_RUNTIME_DIR` is not available, returns an error.
    pub fn get_runtime_file<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        let runtime_dir = self.get_runtime_directory()?;
        Ok(runtime_dir.join(self.user_prefix.join(path)))
    }

    /// Given a relative path `path`, returns an absolute path in
    /// `XDG_CONFIG_HOME` where a configuration file may be stored.
    /// Leading directories in the returned path are pre-created;
    /// if that is not possible, an error is returned.
    pub fn place_config_file<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        write_file(&self.config_home, self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a data file in `XDG_DATA_HOME`.
    pub fn place_data_file<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        write_file(&self.data_home, self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a cache file in `XDG_CACHE_HOME`.
    pub fn place_cache_file<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        write_file(&self.cache_home, self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// an application state file in `XDG_STATE_HOME`.
    pub fn place_state_file<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        write_file(&self.state_home, self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a runtime file in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns an error.
    pub fn place_runtime_file<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        write_file(self.get_runtime_directory()?, self.user_prefix.join(path))
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// configuration file, or `None`. Searches `XDG_CONFIG_HOME` and then
    /// `XDG_CONFIG_DIRS`.
    pub fn find_config_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(&self.config_home, &self.config_dirs,
                  &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, returns an iterator yielding absolute
    /// paths to existing configuration files, in `XDG_CONFIG_DIRS` and
    /// `XDG_CONFIG_HOME`. Paths are produced in order from lowest priority
    /// to highest.
    pub fn find_config_files<P>(&self, path: P) -> FileFindIterator
            where P: AsRef<Path> {
        FileFindIterator::new(&self.config_home, &self.config_dirs,
                    &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// data file, or `None`. Searches `XDG_DATA_HOME` and then
    /// `XDG_DATA_DIRS`.
    pub fn find_data_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(&self.data_home, &self.data_dirs,
                  &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, returns an iterator yielding absolute
    /// paths to existing data files, in `XDG_DATA_DIRS` and
    /// `XDG_DATA_HOME`. Paths are produced in order from lowest priority
    /// to highest.
    pub fn find_data_files<P>(&self, path: P) -> FileFindIterator
            where P: AsRef<Path> {
        FileFindIterator::new(&self.data_home, &self.data_dirs,
                    &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// cache file, or `None`. Searches `XDG_CACHE_HOME`.
    pub fn find_cache_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(&self.cache_home, &Vec::new(),
                  &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// application state file, or `None`. Searches `XDG_STATE_HOME`.
    pub fn find_state_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        read_file(&self.state_home, &Vec::new(),
                  &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// runtime file, or `None`. Searches `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns `None`.
    pub fn find_runtime_file<P>(&self, path: P) -> Option<PathBuf>
            where P: AsRef<Path> {
        if let Ok(runtime_dir) = self.get_runtime_directory() {
            read_file(runtime_dir, &Vec::new(),
                      &self.user_prefix, &self.shared_prefix, path.as_ref())
        } else {
            None
        }
    }

    /// Given a relative path `path`, returns an absolute path to a configuration
    /// directory in `XDG_CONFIG_HOME`. The directory and all directories
    /// leading to it are created if they did not exist;
    /// if that is not possible, an error is returned.
    pub fn create_config_directory<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        create_directory(&self.config_home,
                         self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a data directory in `XDG_DATA_HOME`.
    pub fn create_data_directory<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        create_directory(&self.data_home,
                         self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a cache directory in `XDG_CACHE_HOME`.
    pub fn create_cache_directory<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        create_directory(&self.cache_home,
                         self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for an application state directory in `XDG_STATE_HOME`.
    pub fn create_state_directory<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        create_directory(&self.state_home,
                         self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a runtime directory in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns an error.
    pub fn create_runtime_directory<P>(&self, path: P) -> io::Result<PathBuf>
            where P: AsRef<Path> {
        create_directory(self.get_runtime_directory()?,
                         self.user_prefix.join(path))
    }

    /// Given a relative path `path`, list absolute paths to all files
    /// in directories with path `path` in `XDG_CONFIG_HOME` and
    /// `XDG_CONFIG_DIRS`.
    pub fn list_config_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(&self.config_home, &self.config_dirs,
                   &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Like [`list_config_files`](#method.list_config_files), but
    /// only the first occurence of every distinct filename is returned.
    pub fn list_config_files_once<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files_once(&self.config_home, &self.config_dirs,
                        &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_DATA_HOME` and
    /// `XDG_DATA_DIRS`.
    pub fn list_data_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(&self.data_home, &self.data_dirs,
                   &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Like [`list_data_files`](#method.list_data_files), but
    /// only the first occurence of every distinct filename is returned.
    pub fn list_data_files_once<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files_once(&self.data_home, &self.data_dirs,
                        &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_CACHE_HOME`.
    pub fn list_cache_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(&self.cache_home, &Vec::new(),
                   &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_STATE_HOME`.
    pub fn list_state_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        list_files(&self.state_home, &Vec::new(),
                   &self.user_prefix, &self.shared_prefix, path.as_ref())
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns an empty `Vec`.
    pub fn list_runtime_files<P>(&self, path: P) -> Vec<PathBuf>
            where P: AsRef<Path> {
        if let Ok(runtime_dir) = self.get_runtime_directory() {
            list_files(runtime_dir, &Vec::new(),
                       &self.user_prefix, &self.shared_prefix, path.as_ref())
        } else {
            Vec::new()
        }
    }

    /// Returns the user-specific data directory (set by `XDG_DATA_HOME`).
    pub fn get_data_home(&self) -> PathBuf {
        self.data_home.join(&self.user_prefix)
    }

    /// Returns the user-specific configuration directory (set by
    /// `XDG_CONFIG_HOME`).
    pub fn get_config_home(&self) -> PathBuf {
        self.config_home.join(&self.user_prefix)
    }

    /// Returns the user-specific directory for non-essential (cached) data
    /// (set by `XDG_CACHE_HOME`).
    pub fn get_cache_home(&self) -> PathBuf {
        self.cache_home.join(&self.user_prefix)
    }

    /// Returns the user-specific directory for application state data
    /// (set by `XDG_STATE_HOME`).
    pub fn get_state_home(&self) -> PathBuf {
        self.state_home.join(&self.user_prefix)
    }

    /// Returns a preference ordered (preferred to less preferred) list of
    /// supplementary data directories, ordered by preference (set by
    /// `XDG_DATA_DIRS`).
    pub fn get_data_dirs(&self) -> Vec<PathBuf> {
        self.data_dirs.iter().map(|p| p.join(&self.shared_prefix)).collect()
    }

    /// Returns a preference ordered (preferred to less preferred) list of
    /// supplementary configuration directories (set by `XDG_CONFIG_DIRS`).
    pub fn get_config_dirs(&self) -> Vec<PathBuf> {
        self.config_dirs.iter().map(|p| p.join(&self.shared_prefix)).collect()
    }
}