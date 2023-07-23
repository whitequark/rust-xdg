use std::collections::HashSet;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{env, error, fmt, fs, io};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use self::ErrorKind::*;

/// BaseDirectories allows to look up paths to configuration, data,
/// cache and runtime files in well-known locations according to
/// the [X Desktop Group Base Directory specification][xdg-basedir].
///
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
/// let xdg_dirs = xdg::BaseDirectories::with_prefix("myapp").unwrap();
/// ```
///
/// To store configuration:
///
/// ```
/// # use std::fs::File;
/// # use std::io::{Error, Write};
/// # fn main() -> Result<(), Error> {
/// # let xdg_dirs = xdg::BaseDirectories::with_prefix("myapp").unwrap();
/// let config_path = xdg_dirs
///     .place_config_file("config.ini")
///     .expect("cannot create configuration directory");
/// let mut config_file = File::create(config_path)?;
/// write!(&mut config_file, "configured = 1")?;
/// #   Ok(())
/// # }
/// ```
///
/// The `config.ini` file will appear in the proper location for desktop
/// configuration files, most likely `~/.config/myapp/config.ini`.
/// The leading directories will be automatically created.
///
/// To retrieve supplementary data:
///
/// ```no_run
/// # use std::fs::File;
/// # use std::io::{Error, Read, Write};
/// # fn main() -> Result<(), Error> {
/// # let xdg_dirs = xdg::BaseDirectories::with_prefix("myapp").unwrap();
/// let logo_path = xdg_dirs
///     .find_data_file("logo.png")
///     .expect("application data not present");
/// let mut logo_file = File::open(logo_path)?;
/// let mut logo = Vec::new();
/// logo_file.read_to_end(&mut logo)?;
/// #   Ok(())
/// # }
/// ```
///
/// The `logo.png` will be searched in the proper locations for
/// supplementary data files, most likely `~/.local/share/myapp/logo.png`,
/// then `/usr/local/share/myapp/logo.png` and `/usr/share/myapp/logo.png`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

pub struct Error {
    kind: ErrorKind,
}

impl Error {
    fn new(kind: ErrorKind) -> Error {
        Error { kind }
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match self.kind {
            HomeMissing => "$HOME must be set",
            XdgRuntimeDirInaccessible(_, _) => {
                "$XDG_RUNTIME_DIR must be accessible by the current user"
            }
            XdgRuntimeDirInsecure(_, _) => "$XDG_RUNTIME_DIR must be secure: have permissions 0700",
            XdgRuntimeDirMissing => "$XDG_RUNTIME_DIR is not set",
        }
    }
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.kind {
            XdgRuntimeDirInaccessible(_, ref e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            HomeMissing => write!(f, "$HOME must be set"),
            XdgRuntimeDirInaccessible(ref dir, ref error) => {
                write!(
                    f,
                    "$XDG_RUNTIME_DIR (`{}`) must be accessible \
                           by the current user (error: {})",
                    dir.display(),
                    error
                )
            }
            XdgRuntimeDirInsecure(ref dir, permissions) => {
                write!(
                    f,
                    "$XDG_RUNTIME_DIR (`{}`) must be secure: must have \
                           permissions 0o700, got {}",
                    dir.display(),
                    permissions
                )
            }
            XdgRuntimeDirMissing => {
                write!(f, "$XDG_RUNTIME_DIR must be set")
            }
        }
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> io::Error {
        match error.kind {
            HomeMissing | XdgRuntimeDirMissing => io::Error::new(io::ErrorKind::NotFound, error),
            _ => io::Error::new(io::ErrorKind::Other, error),
        }
    }
}

#[derive(Copy, Clone)]
struct Permissions(u32);

impl fmt::Debug for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Permissions(p) = *self;
        write!(f, "{:#05o}", p)
    }
}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug)]
enum ErrorKind {
    HomeMissing,
    XdgRuntimeDirInaccessible(PathBuf, io::Error),
    XdgRuntimeDirInsecure(PathBuf, Permissions),
    XdgRuntimeDirMissing,
}

impl BaseDirectories {
    /// Reads the process environment, determines the XDG base directories,
    /// and returns a value that can be used for lookup.
    /// The following environment variables are examined:
    ///
    ///   * `HOME`; if not set: use the same fallback as `std::env::home_dir()`;
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
    pub fn new() -> Result<BaseDirectories, Error> {
        BaseDirectories::with_env("", "", &|name| env::var_os(name))
    }

    /// Same as [`new()`](#method.new), but `prefix` is implicitly prepended to
    /// every path that is looked up.
    pub fn with_prefix<P: AsRef<Path>>(prefix: P) -> Result<BaseDirectories, Error> {
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
    /// ```
    /// # extern crate xdg;
    /// # use xdg::BaseDirectories;
    /// let dirs = BaseDirectories::with_profile("program-name", "profile-name").unwrap();
    /// dirs.find_data_file("bar.jpg");
    /// dirs.find_config_file("foo.conf");
    /// ```
    ///
    /// will find `/usr/share/program-name/bar.jpg` (without `profile-name`)
    /// and `~/.config/program-name/profile-name/foo.conf`.
    pub fn with_profile<P1, P2>(prefix: P1, profile: P2) -> Result<BaseDirectories, Error>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        BaseDirectories::with_env(prefix, profile, &|name| env::var_os(name))
    }

    fn with_env<P1, P2, T: ?Sized>(
        prefix: P1,
        profile: P2,
        env_var: &T,
    ) -> Result<BaseDirectories, Error>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
        T: Fn(&str) -> Option<OsString>,
    {
        BaseDirectories::with_env_impl(prefix.as_ref(), profile.as_ref(), env_var)
    }

    fn with_env_impl<T: ?Sized>(
        prefix: &Path,
        profile: &Path,
        env_var: &T,
    ) -> Result<BaseDirectories, Error>
    where
        T: Fn(&str) -> Option<OsString>,
    {
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
                .filter(|path| path.is_absolute())
                .collect::<Vec<_>>();
            if paths.is_empty() {
                None
            } else {
                Some(paths)
            }
        }

        // This crate only supports Unix, and the behavior of `std::env::home_dir()` is only
        // problematic on Windows.
        #[allow(deprecated)]
        let home = std::env::home_dir().ok_or(Error::new(HomeMissing))?;

        let data_home = env_var("XDG_DATA_HOME")
            .and_then(abspath)
            .unwrap_or(home.join(".local/share"));
        let config_home = env_var("XDG_CONFIG_HOME")
            .and_then(abspath)
            .unwrap_or(home.join(".config"));
        let cache_home = env_var("XDG_CACHE_HOME")
            .and_then(abspath)
            .unwrap_or(home.join(".cache"));
        let state_home = env_var("XDG_STATE_HOME")
            .and_then(abspath)
            .unwrap_or(home.join(".local/state"));
        let data_dirs = env_var("XDG_DATA_DIRS").and_then(abspaths).unwrap_or(vec![
            PathBuf::from("/usr/local/share"),
            PathBuf::from("/usr/share"),
        ]);
        let config_dirs = env_var("XDG_CONFIG_DIRS")
            .and_then(abspaths)
            .unwrap_or(vec![PathBuf::from("/etc/xdg")]);
        let runtime_dir = env_var("XDG_RUNTIME_DIR").and_then(abspath); // optional

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
    pub fn get_runtime_directory(&self) -> Result<&PathBuf, Error> {
        if let Some(ref runtime_dir) = self.runtime_dir {
            // If XDG_RUNTIME_DIR is in the environment but not secure,
            // do not allow recovery.
            fs::read_dir(runtime_dir)
                .map_err(|e| Error::new(XdgRuntimeDirInaccessible(runtime_dir.clone(), e)))?;
            let permissions = fs::metadata(runtime_dir)
                .map_err(|e| Error::new(XdgRuntimeDirInaccessible(runtime_dir.clone(), e)))?
                .permissions()
                .mode();
            if permissions & 0o077 != 0 {
                Err(Error::new(XdgRuntimeDirInsecure(
                    runtime_dir.clone(),
                    Permissions(permissions),
                )))
            } else {
                Ok(runtime_dir)
            }
        } else {
            Err(Error::new(XdgRuntimeDirMissing))
        }
    }

    /// Returns `true` if `XDG_RUNTIME_DIR` is available, `false` otherwise.
    pub fn has_runtime_directory(&self) -> bool {
        self.get_runtime_directory().is_ok()
    }

    /// Like [`place_config_file()`](#method.place_config_file), but does
    /// not create any directories.
    pub fn get_config_file<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.config_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_data_file()`](#method.place_data_file), but does
    /// not create any directories.
    pub fn get_data_file<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.data_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_cache_file()`](#method.place_cache_file), but does
    /// not create any directories.
    pub fn get_cache_file<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.cache_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_state_file()`](#method.place_state_file), but does
    /// not create any directories.
    pub fn get_state_file<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.state_home.join(self.user_prefix.join(path))
    }

    /// Like [`place_runtime_file()`](#method.place_runtime_file), but does
    /// not create any directories.
    /// If `XDG_RUNTIME_DIR` is not available, returns an error.
    pub fn get_runtime_file<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        let runtime_dir = self.get_runtime_directory()?;
        Ok(runtime_dir.join(self.user_prefix.join(path)))
    }

    /// Given a relative path `path`, returns an absolute path in
    /// `XDG_CONFIG_HOME` where a configuration file may be stored.
    /// Leading directories in the returned path are pre-created;
    /// if that is not possible, an error is returned.
    pub fn place_config_file<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        write_file(&self.config_home, &self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a data file in `XDG_DATA_HOME`.
    pub fn place_data_file<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        write_file(&self.data_home, &self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a cache file in `XDG_CACHE_HOME`.
    pub fn place_cache_file<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        write_file(&self.cache_home, &self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// an application state file in `XDG_STATE_HOME`.
    pub fn place_state_file<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        write_file(&self.state_home, &self.user_prefix.join(path))
    }

    /// Like [`place_config_file()`](#method.place_config_file), but for
    /// a runtime file in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns an error.
    pub fn place_runtime_file<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        write_file(self.get_runtime_directory()?, &self.user_prefix.join(path))
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// configuration file, or `None`. Searches `XDG_CONFIG_HOME` and then
    /// `XDG_CONFIG_DIRS`.
    pub fn find_config_file<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        read_file(
            &self.config_home,
            &self.config_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, returns an iterator yielding absolute
    /// paths to existing configuration files, in `XDG_CONFIG_DIRS` and
    /// `XDG_CONFIG_HOME`. Paths are produced in order from lowest priority
    /// to highest.
    pub fn find_config_files<P: AsRef<Path>>(&self, path: P) -> FileFindIterator {
        FileFindIterator::new(
            &self.config_home,
            &self.config_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// data file, or `None`. Searches `XDG_DATA_HOME` and then
    /// `XDG_DATA_DIRS`.
    pub fn find_data_file<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        read_file(
            &self.data_home,
            &self.data_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, returns an iterator yielding absolute
    /// paths to existing data files, in `XDG_DATA_DIRS` and
    /// `XDG_DATA_HOME`. Paths are produced in order from lowest priority
    /// to highest.
    pub fn find_data_files<P: AsRef<Path>>(&self, path: P) -> FileFindIterator {
        FileFindIterator::new(
            &self.data_home,
            &self.data_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// cache file, or `None`. Searches `XDG_CACHE_HOME`.
    pub fn find_cache_file<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        read_file(
            &self.cache_home,
            &Vec::new(),
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// application state file, or `None`. Searches `XDG_STATE_HOME`.
    pub fn find_state_file<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        read_file(
            &self.state_home,
            &Vec::new(),
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, returns an absolute path to an existing
    /// runtime file, or `None`. Searches `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns `None`.
    pub fn find_runtime_file<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        if let Ok(runtime_dir) = self.get_runtime_directory() {
            read_file(
                runtime_dir,
                &Vec::new(),
                &self.user_prefix,
                &self.shared_prefix,
                path.as_ref(),
            )
        } else {
            None
        }
    }

    /// Given a relative path `path`, returns an absolute path to a configuration
    /// directory in `XDG_CONFIG_HOME`. The directory and all directories
    /// leading to it are created if they did not exist;
    /// if that is not possible, an error is returned.
    pub fn create_config_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        create_directory(&self.config_home, &self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a data directory in `XDG_DATA_HOME`.
    pub fn create_data_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        create_directory(&self.data_home, &self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a cache directory in `XDG_CACHE_HOME`.
    pub fn create_cache_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        create_directory(&self.cache_home, &self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for an application state directory in `XDG_STATE_HOME`.
    pub fn create_state_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        create_directory(&self.state_home, &self.user_prefix.join(path))
    }

    /// Like [`create_config_directory()`](#method.create_config_directory),
    /// but for a runtime directory in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns an error.
    pub fn create_runtime_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<PathBuf> {
        create_directory(self.get_runtime_directory()?, &self.user_prefix.join(path))
    }

    /// Given a relative path `path`, list absolute paths to all files
    /// in directories with path `path` in `XDG_CONFIG_HOME` and
    /// `XDG_CONFIG_DIRS`.
    pub fn list_config_files<P: AsRef<Path>>(&self, path: P) -> Vec<PathBuf> {
        list_files(
            &self.config_home,
            &self.config_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Like [`list_config_files`](#method.list_config_files), but
    /// only the first occurence of every distinct filename is returned.
    pub fn list_config_files_once<P: AsRef<Path>>(&self, path: P) -> Vec<PathBuf> {
        list_files_once(
            &self.config_home,
            &self.config_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_DATA_HOME` and
    /// `XDG_DATA_DIRS`.
    pub fn list_data_files<P: AsRef<Path>>(&self, path: P) -> Vec<PathBuf> {
        list_files(
            &self.data_home,
            &self.data_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Like [`list_data_files`](#method.list_data_files), but
    /// only the first occurence of every distinct filename is returned.
    pub fn list_data_files_once<P: AsRef<Path>>(&self, path: P) -> Vec<PathBuf> {
        list_files_once(
            &self.data_home,
            &self.data_dirs,
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_CACHE_HOME`.
    pub fn list_cache_files<P: AsRef<Path>>(&self, path: P) -> Vec<PathBuf> {
        list_files(
            &self.cache_home,
            &Vec::new(),
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_STATE_HOME`.
    pub fn list_state_files<P: AsRef<Path>>(&self, path: P) -> Vec<PathBuf> {
        list_files(
            &self.state_home,
            &Vec::new(),
            &self.user_prefix,
            &self.shared_prefix,
            path.as_ref(),
        )
    }

    /// Given a relative path `path`, lists absolute paths to all files
    /// in directories with path `path` in `XDG_RUNTIME_DIR`.
    /// If `XDG_RUNTIME_DIR` is not available, returns an empty `Vec`.
    pub fn list_runtime_files<P: AsRef<Path>>(&self, path: P) -> Vec<PathBuf> {
        if let Ok(runtime_dir) = self.get_runtime_directory() {
            list_files(
                runtime_dir,
                &Vec::new(),
                &self.user_prefix,
                &self.shared_prefix,
                path.as_ref(),
            )
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
        self.data_dirs
            .iter()
            .map(|p| p.join(&self.shared_prefix))
            .collect()
    }

    /// Returns a preference ordered (preferred to less preferred) list of
    /// supplementary configuration directories (set by `XDG_CONFIG_DIRS`).
    pub fn get_config_dirs(&self) -> Vec<PathBuf> {
        self.config_dirs
            .iter()
            .map(|p| p.join(&self.shared_prefix))
            .collect()
    }
}

fn write_file(home: &Path, path: &Path) -> io::Result<PathBuf> {
    match path.parent() {
        Some(parent) => fs::create_dir_all(home.join(parent))?,
        None => fs::create_dir_all(home)?,
    }
    Ok(home.join(path))
}

fn create_directory(home: &Path, path: &Path) -> io::Result<PathBuf> {
    let full_path = home.join(path);
    fs::create_dir_all(&full_path)?;
    Ok(full_path)
}

fn path_exists(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

fn read_file(
    home: &Path,
    dirs: &[PathBuf],
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
    fn new(
        home: &Path,
        dirs: &[PathBuf],
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
            let candidate = dir.join(&self.relpath);
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
            let candidate = dir.join(&self.relpath);
            if path_exists(&candidate) {
                return Some(candidate);
            }
        }
    }
}

fn list_files(
    home: &Path,
    dirs: &[PathBuf],
    user_prefix: &Path,
    shared_prefix: &Path,
    path: &Path,
) -> Vec<PathBuf> {
    fn read_dir(dir: &Path, into: &mut Vec<PathBuf>) {
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

fn list_files_once(
    home: &Path,
    dirs: &[PathBuf],
    user_prefix: &Path,
    shared_prefix: &Path,
    path: &Path,
) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    list_files(home, dirs, user_prefix, shared_prefix, path)
        .into_iter()
        .filter(|path| match path.file_name() {
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

#[cfg(test)]
mod test {
    use super::*;

    const TARGET_TMPDIR: Option<&'static str> = option_env!("CARGO_TARGET_TMPDIR");
    const TARGET_DIR: Option<&'static str> = option_env!("CARGO_TARGET_DIR");

    fn get_test_dir() -> PathBuf {
        match TARGET_TMPDIR {
            Some(dir) => PathBuf::from(dir),
            None => match TARGET_DIR {
                Some(dir) => PathBuf::from(dir),
                None => env::current_dir().unwrap(),
            },
        }
    }

    fn path_exists<P: AsRef<Path> + ?Sized>(path: &P) -> bool {
        super::path_exists(path.as_ref())
    }

    fn path_is_dir<P: ?Sized + AsRef<Path>>(path: &P) -> bool {
        fn inner(path: &Path) -> bool {
            fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
        }
        inner(path.as_ref())
    }

    fn make_absolute<P: AsRef<Path>>(path: P) -> PathBuf {
        get_test_dir().join(path.as_ref())
    }

    fn iter_after<A, I, J>(mut iter: I, mut prefix: J) -> Option<I>
    where
        I: Iterator<Item = A> + Clone,
        J: Iterator<Item = A>,
        A: PartialEq,
    {
        loop {
            let mut iter_next = iter.clone();
            match (iter_next.next(), prefix.next()) {
                (Some(x), Some(y)) => {
                    if x != y {
                        return None;
                    }
                }
                (Some(_), None) => return Some(iter),
                (None, None) => return Some(iter),
                (None, Some(_)) => return None,
            }
            iter = iter_next;
        }
    }

    fn make_relative<P: AsRef<Path>>(path: P, reference: P) -> PathBuf {
        iter_after(path.as_ref().components(), reference.as_ref().components())
            .unwrap()
            .as_path()
            .to_owned()
    }

    fn make_env(vars: Vec<(&'static str, String)>) -> Box<dyn Fn(&str) -> Option<OsString>> {
        return Box::new(move |name| {
            for &(key, ref value) in vars.iter() {
                if key == name {
                    return Some(OsString::from(value));
                }
            }
            None
        });
    }

    #[test]
    fn test_files_exists() {
        assert!(path_exists("test_files"));
        assert!(
            fs::metadata("test_files/runtime-bad")
                .unwrap()
                .permissions()
                .mode()
                & 0o077
                != 0
        );
    }

    #[test]
    fn test_bad_environment() {
        let xd = BaseDirectories::with_env(
            "",
            "",
            &*make_env(vec![
                ("HOME", "test_files/user".to_string()),
                ("XDG_DATA_HOME", "test_files/user/data".to_string()),
                ("XDG_CONFIG_HOME", "test_files/user/config".to_string()),
                ("XDG_CACHE_HOME", "test_files/user/cache".to_string()),
                ("XDG_DATA_DIRS", "test_files/user/data".to_string()),
                ("XDG_CONFIG_DIRS", "test_files/user/config".to_string()),
                ("XDG_RUNTIME_DIR", "test_files/runtime-bad".to_string()),
            ]),
        )
        .unwrap();
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
        assert_eq!(
            config_files.next(),
            Some(PathBuf::from(format!(
                "{}/test_files/system2/config/everywhere",
                cwd
            )))
        );
        assert_eq!(
            config_files.next(),
            Some(PathBuf::from(format!(
                "{}/test_files/system1/config/everywhere",
                cwd
            )))
        );
        assert_eq!(
            config_files.next(),
            Some(PathBuf::from(format!(
                "{}/test_files/user/config/everywhere",
                cwd
            )))
        );
        assert_eq!(config_files.next(), None);

        let mut data_files = xd.find_data_files("everywhere");
        assert_eq!(
            data_files.next(),
            Some(PathBuf::from(format!(
                "{}/test_files/system2/data/everywhere",
                cwd
            )))
        );
        assert_eq!(
            data_files.next(),
            Some(PathBuf::from(format!(
                "{}/test_files/system1/data/everywhere",
                cwd
            )))
        );
        assert_eq!(
            data_files.next(),
            Some(PathBuf::from(format!(
                "{}/test_files/user/data/everywhere",
                cwd
            )))
        );
        assert_eq!(data_files.next(), None);
    }

    #[test]
    fn test_runtime_bad() {
        let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
        let xd = BaseDirectories::with_env(
            "",
            "",
            &*make_env(vec![
                ("HOME", format!("{}/test_files/user", cwd)),
                ("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
            ]),
        )
        .unwrap();
        assert!(xd.has_runtime_directory() == false);
    }

    #[test]
    fn test_runtime_good() {
        use std::fs::File;

        let test_runtime_dir = make_absolute(&"test_files/runtime-good");
        fs::create_dir_all(&test_runtime_dir).unwrap();

        let mut perms = fs::metadata(&test_runtime_dir).unwrap().permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&test_runtime_dir, perms).unwrap();

        let test_dir = get_test_dir().to_string_lossy().into_owned();
        let xd = BaseDirectories::with_env(
            "",
            "",
            &*make_env(vec![
                ("HOME", format!("{}/test_files/user", test_dir)),
                (
                    "XDG_RUNTIME_DIR",
                    format!("{}/test_files/runtime-good", test_dir),
                ),
            ]),
        )
        .unwrap();

        xd.create_runtime_directory("foo").unwrap();
        assert!(path_is_dir(&format!(
            "{}/test_files/runtime-good/foo",
            test_dir
        )));
        let w = xd.place_runtime_file("bar/baz").unwrap();
        assert!(path_is_dir(&format!(
            "{}/test_files/runtime-good/bar",
            test_dir
        )));
        assert!(!path_exists(&format!(
            "{}/test_files/runtime-good/bar/baz",
            test_dir
        )));
        File::create(&w).unwrap();
        assert!(path_exists(&format!(
            "{}/test_files/runtime-good/bar/baz",
            test_dir
        )));
        assert!(xd.find_runtime_file("bar/baz") == Some(w.clone()));
        File::open(&w).unwrap();
        fs::remove_file(&w).unwrap();
        let root = xd.list_runtime_files(".");
        let mut root = root
            .into_iter()
            .map(|p| make_relative(&p, &get_test_dir()))
            .collect::<Vec<_>>();
        root.sort();
        assert_eq!(
            root,
            vec![
                PathBuf::from("test_files/runtime-good/bar"),
                PathBuf::from("test_files/runtime-good/foo")
            ]
        );
        assert!(xd.list_runtime_files("bar").is_empty());
        assert!(xd.find_runtime_file("foo/qux").is_none());
        assert!(xd.find_runtime_file("qux/foo").is_none());
        assert!(!path_exists(&format!(
            "{}/test_files/runtime-good/qux",
            test_dir
        )));
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
        let mut files = files
            .into_iter()
            .map(|p| make_relative(&p, &env::current_dir().unwrap()))
            .collect::<Vec<_>>();
        files.sort();
        assert_eq!(
            files,
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
            ]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
        );

        let files = xd.list_config_files_once(".");
        let mut files = files
            .into_iter()
            .map(|p| make_relative(&p, &env::current_dir().unwrap()))
            .collect::<Vec<_>>();
        files.sort();
        assert_eq!(
            files,
            [
                "test_files/system1/config/both_system_config.file",
                "test_files/system1/config/system1_config.file",
                "test_files/system2/config/system2_config.file",
                "test_files/user/config/everywhere",
                "test_files/user/config/myapp",
                "test_files/user/config/user_config.file",
            ]
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_get_file() {
        let test_dir = get_test_dir().to_string_lossy().into_owned();
        let xd = BaseDirectories::with_env(
            "",
            "",
            &*make_env(vec![
                ("HOME", format!("{}/test_files/user", test_dir)),
                (
                    "XDG_DATA_HOME",
                    format!("{}/test_files/user/data", test_dir),
                ),
                (
                    "XDG_CONFIG_HOME",
                    format!("{}/test_files/user/config", test_dir),
                ),
                (
                    "XDG_CACHE_HOME",
                    format!("{}/test_files/user/cache", test_dir),
                ),
                (
                    "XDG_RUNTIME_DIR",
                    format!("{}/test_files/user/runtime", test_dir),
                ),
            ]),
        )
        .unwrap();

        let path = format!("{}/test_files/user/runtime/", test_dir);
        fs::create_dir_all(&path).unwrap();
        let metadata = fs::metadata(&path).expect("Could not read metadata for runtime directory");
        let mut perms = metadata.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&path, perms).expect("Could not set permissions for runtime directory");

        let file = xd.get_config_file("myapp/user_config.file");
        assert_eq!(
            file,
            PathBuf::from(&format!(
                "{}/test_files/user/config/myapp/user_config.file",
                test_dir
            ))
        );

        let file = xd.get_data_file("user_data.file");
        assert_eq!(
            file,
            PathBuf::from(&format!("{}/test_files/user/data/user_data.file", test_dir))
        );

        let file = xd.get_cache_file("user_cache.file");
        assert_eq!(
            file,
            PathBuf::from(&format!(
                "{}/test_files/user/cache/user_cache.file",
                test_dir
            ))
        );

        let file = xd.get_runtime_file("user_runtime.file").unwrap();
        assert_eq!(
            file,
            PathBuf::from(&format!(
                "{}/test_files/user/runtime/user_runtime.file",
                test_dir
            ))
        );
    }

    #[test]
    fn test_prefix() {
        let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
        let xd = BaseDirectories::with_env(
            "myapp",
            "",
            &*make_env(vec![
                ("HOME", format!("{}/test_files/user", cwd)),
                ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ]),
        )
        .unwrap();
        assert_eq!(
            xd.get_cache_file("cache.db"),
            PathBuf::from(&format!("{}/test_files/user/cache/myapp/cache.db", cwd))
        );
        assert_eq!(
            xd.place_cache_file("cache.db").unwrap(),
            PathBuf::from(&format!("{}/test_files/user/cache/myapp/cache.db", cwd))
        );
    }

    #[test]
    fn test_profile() {
        let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
        let xd = BaseDirectories::with_env(
            "myapp",
            "default_profile",
            &*make_env(vec![
                ("HOME", format!("{}/test_files/user", cwd)),
                ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
                (
                    "XDG_CONFIG_DIRS",
                    format!("{}/test_files/system1/config", cwd),
                ),
            ]),
        )
        .unwrap();
        assert_eq!(
            xd.find_config_file("system1_config.file").unwrap(),
            // Does *not* include default_profile
            PathBuf::from(&format!(
                "{}/test_files/system1/config/myapp/system1_config.file",
                cwd
            ))
        );
        assert_eq!(
            xd.find_config_file("user_config.file").unwrap(),
            // Includes default_profile
            PathBuf::from(&format!(
                "{}/test_files/user/config/myapp/default_profile/user_config.file",
                cwd
            ))
        );
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
            "myapp",
            "",
            &*make_env(vec![
                ("HOME", symlinks_dir),
                ("XDG_CONFIG_HOME", config_dir),
            ]),
        )
        .unwrap();
        assert_eq!(
            xd.find_config_file("user_config.file").unwrap(),
            PathBuf::from(&format!("{}/user_config.file", myapp_dir))
        );
    }
}
