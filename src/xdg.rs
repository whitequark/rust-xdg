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

    pub fn place_data_file<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        write_file(&self.data_home, path)
    }
    pub fn place_config_file<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        write_file(&self.config_home, path)
    }
    pub fn place_cache_file<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        write_file(&self.cache_home, path)
    }
    pub fn place_runtime_file<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        write_file(self.runtime_dir.as_ref().unwrap(), path)
    }

    pub fn find_data_file<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        read_file(&self.data_home, &self.data_dirs, path)
    }
    pub fn find_config_file<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        read_file(&self.config_home, &self.config_dirs, path)
    }
    pub fn find_cache_file<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        read_file(&self.cache_home, &Vec::new(), path)
    }
    pub fn find_runtime_file<P>(&self, path: P) -> Option<PathBuf> where P: AsRef<Path> {
        read_file(self.runtime_dir.as_ref().unwrap(), &Vec::new(), path)
    }

    pub fn create_data_directory<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        create_directory(&self.data_home, path)
    }
    pub fn create_config_directory<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        create_directory(&self.config_home, path)
    }
    pub fn create_cache_directory<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        create_directory(&self.cache_home, path)
    }
    pub fn create_runtime_directory<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
        create_directory(self.runtime_dir.as_ref().unwrap(), path)
    }

    pub fn list_data_files<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        list_files(&self.data_home, &self.data_dirs, path)
    }
    pub fn list_data_files_once<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        list_files_once(&self.data_home, &self.data_dirs, path)
    }
    pub fn list_config_files<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        list_files(&self.config_home, &self.config_dirs, path)
    }
    pub fn list_config_files_once<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        list_files_once(&self.config_home, &self.config_dirs, path)
    }
    pub fn list_cache_files<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        list_files(&self.cache_home, &Vec::new(), path)
    }
    pub fn list_runtime_files<P>(&self, path: P) -> Vec<PathBuf> where P: AsRef<Path> {
        list_files(self.runtime_dir.as_ref().unwrap(), &Vec::new(), path)
    }
}

fn write_file<P>(home: &Path, path: P) -> PathBuf
        where P: AsRef<Path> {
    match path.as_ref().parent() {
        Some(parent) => fs::create_dir_all(home.join(parent)).unwrap(),
        None => fs::create_dir_all(home).unwrap(),
    }
    PathBuf::from(home.join(path.as_ref()))
}

fn create_directory<P>(home: &Path, path: P) -> PathBuf
        where P: AsRef<Path> {
    let full_path = home.join(path.as_ref());
    fs::create_dir_all(&full_path).unwrap();
    full_path
}

fn read_file<P>(home: &Path, dirs: &Vec<PathBuf>, path: P) -> Option<PathBuf>
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

fn list_files<P>(home: &Path, dirs: &Vec<PathBuf>, path: P) -> Vec<PathBuf>
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

fn list_files_once<P>(home: &Path, dirs: &Vec<PathBuf>, path: P) -> Vec<PathBuf>
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
            // ("XDG_RUNTIME_DIR", "test_files/runtime-bad".to_string())
        ]));
    assert_eq!(xd.find_data_file("everywhere"), None);
    assert_eq!(xd.find_config_file("everywhere"), None);
    assert_eq!(xd.find_cache_file("everywhere"), None);
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
    assert!(xd.find_data_file("everywhere") != None);
    assert!(xd.find_config_file("everywhere") != None);
    assert!(xd.find_cache_file("everywhere") != None);
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
    xd.create_runtime_directory("foo");
    assert!(Path::new("test_files/runtime-good/foo").is_dir());
    let w = xd.place_runtime_file("bar/baz");
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
    let xd = XdgDirs::new_with_env(&*make_env(vec![
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
            //("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
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
            "test_files/user/config/./everywhere",
            "test_files/user/config/./user_config.file",
        ].iter().map(PathBuf::from).collect::<Vec<_>>());
}
