#![feature(path_ext, path_relative_from)]

use std::path::{Path, PathBuf};
use std::env;
use std::fs;

use std::fs::PathExt;
use std::os::unix::fs::PermissionsExt;

pub struct XdgDirs {
    data_home: PathBuf,
    config_home: PathBuf,
    cache_home: PathBuf,
    data_dirs: Vec<PathBuf>,
    config_dirs: Vec<PathBuf>,
    runtime_dir: Option<PathBuf>,
}

impl XdgDirs
{
    pub fn new() -> XdgDirs {
        XdgDirs::new_with_env(&|name| env::var(name))
    }

    fn new_with_env<T: ?Sized>(env_var: &T) -> XdgDirs
            where T: Fn(&str) -> Result<String, env::VarError> {
        fn abspath(path: String) -> Option<PathBuf> {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                Some(path)
            } else {
                None
            }
        }

        fn abspaths(paths: String) -> Option<Vec<PathBuf>> {
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

        let home = env::home_dir().expect("$HOME must be set");

        let data_home   = env_var("XDG_DATA_HOME")
                              .ok().and_then(abspath)
                              .unwrap_or(home.join(".local/share"));
        let config_home = env_var("XDG_CONFIG_HOME")
                              .ok().and_then(abspath)
                              .unwrap_or(home.join(".config"));
        let cache_home  = env_var("XDG_CACHE_HOME")
                              .ok().and_then(abspath)
                              .unwrap_or(home.join(".cache"));
        let data_dirs   = env_var("XDG_DATA_DIRS")
                              .ok().and_then(abspaths)
                              .unwrap_or(vec![PathBuf::from("/usr/local/share"),
                                              PathBuf::from("/usr/share")]);
        let config_dirs = env_var("XDG_CONFIG_DIRS")
                              .ok().and_then(abspaths)
                              .unwrap_or(vec![PathBuf::from("/etc/xdg")]);
        let runtime_dir = env_var("XDG_RUNTIME_DIR")
                              .ok().and_then(abspath); // optional

        // If XDG_RUNTIME_DIR is in the environment but not secure,
        // do not allow recovery.
        match runtime_dir {
            Some(ref p) => {
                match p.metadata() {
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
            None => {
                if env_var("XDG_RUNTIME_DIR").is_ok() {
                    panic!("$XDG_RUNTIME_DIR must be absolute");
                }
            }
        }

        XdgDirs {
            data_home: data_home,
            config_home: config_home,
            cache_home: cache_home,
            data_dirs: data_dirs,
            config_dirs: config_dirs,
            runtime_dir: runtime_dir,
        }
    }

    pub fn want_write_data<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_file(&self.data_home, path)
    }
    pub fn want_write_config<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_file(&self.config_home, path)
    }
    pub fn want_write_cache<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_file(&self.cache_home, path)
    }
    pub fn need_write_runtime<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_file(self.runtime_dir.as_ref().unwrap(), path)
    }
    pub fn want_mkdir_data<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_dir(&self.data_home, path)
    }
    pub fn want_mkdir_config<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_dir(&self.config_home, path)
    }
    pub fn want_mkdir_cache<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_dir(&self.cache_home, path)
    }
    pub fn need_mkdir_runtime<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        want_write_dir(self.runtime_dir.as_ref().unwrap(), path)
    }

    pub fn want_read_data<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        want_read_file(&self.data_home, &self.data_dirs, path)
    }
    pub fn want_read_config<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        want_read_file(&self.config_home, &self.config_dirs, path)
    }
    pub fn want_read_cache<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        want_read_file(&self.cache_home, &Vec::new(), path)
    }
    pub fn need_read_runtime<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        want_read_file(self.runtime_dir.as_ref().unwrap(), &Vec::new(), path)
    }

    pub fn want_list_data_all<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        want_list_file_all(&self.data_home, &self.data_dirs, path)
    }
    pub fn want_list_config_all<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        want_list_file_all(&self.config_home, &self.config_dirs, path)
    }
    pub fn want_list_data_once<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        want_list_file_once(&self.data_home, &self.data_dirs, path)
    }
    pub fn want_list_config_once<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        want_list_file_once(&self.config_home, &self.config_dirs, path)
    }
    pub fn want_list_cache<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        want_list_file_all(&self.cache_home, &Vec::new(), path)
    }
    pub fn need_list_runtime<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        want_list_file_all(self.runtime_dir.as_ref().unwrap(), &Vec::new(), path)
    }
}

fn want_write_file<P>(home: &Path, path: P) -> PathBuf
        where P: AsRef<Path> {
    match path.as_ref().parent() {
        Some(parent) => fs::create_dir_all(home.join(parent)).unwrap(),
        None => fs::create_dir_all(home).unwrap(),
    }
    PathBuf::from(home.join(path.as_ref()))
}

fn want_write_dir<P>(home: &Path, path: P) -> PathBuf
        where P: AsRef<Path> {
    let full_path = home.join(path.as_ref());
    fs::create_dir_all(&full_path).unwrap();
    full_path
}

fn want_read_file<P>(home: &Path, dirs: &Vec<PathBuf>, path: P) -> Option<PathBuf>
        where P: AsRef<Path> {
    let full_path = home.join(path.as_ref());
    if full_path.exists() {
        return Some(full_path);
    }
    for dir in dirs.iter() {
        let full_path = dir.join(path.as_ref());
        if full_path.exists() {
            return Some(full_path);
        }
    }
    None
}

fn want_list_file_all<P>(home: &Path, dirs: &Vec<PathBuf>, path: P) -> Vec<PathBuf>
        where P: AsRef<Path> {
    [home.join(path.as_ref())].iter()
        .chain(dirs.iter())
        .map(|path| {
            fs::read_dir(home.join(path))
               .map(|dir| dir.filter_map(|entry| entry.ok())
                             .map(|entry| entry.path())
                             .collect::<Vec<_>>())
               .unwrap_or(Vec::new())
        })
        .fold(vec![], |mut accum, paths| { accum.extend(paths); accum })
}

fn want_list_file_once<P>(home: &Path, dirs: &Vec<PathBuf>, path: P) -> Vec<PathBuf>
        where P: AsRef<Path> {
    let mut seen = std::collections::HashSet::new();
    want_list_file_all(home, dirs, path).into_iter().filter(|path| {
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
        Box<Fn(&str)->Result<String, env::VarError>> {
    return Box::new(move |name| {
        for &(key, ref value) in vars.iter() {
            if key == name { return Ok(value.clone()) }
        }
        Err(env::VarError::NotPresent)
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
    let xd = XdgDirs::new_with_env(&*make_env(vec![
            ("XDG_DATA_HOME", "test_files/user/data".to_string()),
            ("XDG_CONFIG_HOME", "test_files/user/config".to_string()),
            ("XDG_CACHE_HOME", "test_files/user/cache".to_string()),
            ("XDG_DATA_DIRS", "test_files/user/data".to_string()),
            ("XDG_CONFIG_DIRS", "test_files/user/config".to_string()),
            // ("XDG_RUNTIME_DIR", "test_files/runtime-bad")
        ]));
    assert_eq!(xd.want_read_data("everywhere"), None);
    assert_eq!(xd.want_read_config("everywhere"), None);
    assert_eq!(xd.want_read_cache("everywhere"), None);
}

#[test]
fn test_good_environment() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = XdgDirs::new_with_env(&*make_env(vec![
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
            //("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
        ]));
    assert!(xd.want_read_data("everywhere") != None);
    assert!(xd.want_read_config("everywhere") != None);
    assert!(xd.want_read_cache("everywhere") != None);
}

#[test]
fn test_runtime_bad() {
    let test_runtime_dir = make_absolute(&"test_files/runtime-bad");
    std::thread::spawn(move || {
        let _ = XdgDirs::new_with_env(&|v| {
            if v == "XDG_RUNTIME_DIR" {
                Ok(test_runtime_dir.to_string_lossy().into_owned())
            } else {
                Err(env::VarError::NotPresent)
            }
        });
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

    let xd = XdgDirs::new_with_env(&|v| {
        if v == "XDG_RUNTIME_DIR" {
            Ok(test_runtime_dir.to_string_lossy().into_owned())
        } else {
            Err(env::VarError::NotPresent)
        }
    });
    xd.need_mkdir_runtime("foo");
    assert!(Path::new("test_files/runtime-good/foo").is_dir());
    let w = xd.need_write_runtime("bar/baz");
    assert!(Path::new("test_files/runtime-good/bar").is_dir());
    assert!(!Path::new("test_files/runtime-good/bar/baz").exists());
    File::create(&w).unwrap();
    assert!(Path::new("test_files/runtime-good/bar/baz").exists());
    assert!(xd.need_read_runtime("bar/baz") == Some(w.clone()));
    File::open(&w).unwrap();
    fs::remove_file(&w).unwrap();
    let root = xd.need_list_runtime(".");
    let mut root = root.into_iter().map(|p| make_relative(&p)).collect::<Vec<_>>();
    root.sort();
    assert_eq!(root,
               vec![PathBuf::from("test_files/runtime-good/bar"),
                    PathBuf::from("test_files/runtime-good/foo")]);
    assert!(xd.need_list_runtime("bar").is_empty());
    assert!(xd.need_read_runtime("foo/qux").is_none());
    assert!(xd.need_read_runtime("qux/foo").is_none());
    assert!(!Path::new("test_files/runtime-good/qux").exists());
}

#[test]
fn test_lists() {
    let cwd = env::current_dir().unwrap().to_string_lossy().into_owned();
    let xd = XdgDirs::new_with_env(&*make_env(vec![
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
            //("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
        ]));

    let files = xd.want_list_config_all(".");
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

    let files = xd.want_list_config_once(".");
    let mut files = files.into_iter().map(|p| make_relative(&p)).collect::<Vec<_>>();
    files.sort();
    assert_eq!(files,
        [
            "test_files/system1/config/both_system_config.file",
            "test_files/system1/config/system1_config.file",
            "test_files/system2/config/system2_config.file",
            "test_files/user/config/./everywhere",
            "test_files/user/config/./user_config.file",
        ].iter().map(PathBuf::from).collect::<Vec<_>>());
}
