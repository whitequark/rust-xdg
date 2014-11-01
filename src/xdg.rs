use std::path::BytesContainer;
use std::io;
use std::io::fs::PathExtensions;

pub struct XdgDirs
{
    data_home: Path,
    config_home: Path,
    cache_home: Path,
    data_dirs: Vec<Path>,
    config_dirs: Vec<Path>,
    runtime_dir: Option<Path>,
}

impl XdgDirs
{
    pub fn new() -> XdgDirs
    {
        XdgDirs::new_with_env(std::os::getenv_as_bytes)
    }

    fn new_with_env(env: |&str| -> Option<Vec<u8>>) -> XdgDirs
    {
        let home = std::os::homedir().unwrap();
        if !home.exists()
        {
            panic!("no homeless users allowed");
        }

        let data_home = getenv_one(|n: &str| env(n), "XDG_DATA_HOME").unwrap_or(home.join(".local/share"));
        let config_home = getenv_one(|n: &str| env(n), "XDG_CONFIG_HOME").unwrap_or(home.join(".config"));
        let cache_home = getenv_one(|n: &str| env(n), "XDG_CACHE_HOME").unwrap_or(home.join(".cache"));
        let data_dirs = getenv_many(|n: &str| env(n), "XDG_DATA_DIRS").unwrap_or(vec![Path::new("/usr/local/share"), Path::new("/usr/share")]);
        let config_dirs = getenv_many(|n: &str| env(n), "XDG_CONFIG_DIRS").unwrap_or(vec![Path::new("/etc/xdg")]);
        let runtime_dir = getenv_one(|n: &str| env(n), "XDG_RUNTIME_DIR"); // optional
        match runtime_dir
        {
            // If XDG_RUNTIME_DIR is in the environment but not secure,
            // do not allow recovery.
            Some(ref p) =>
            {
                match p.stat()
                {
                    Err(_) =>
                    {
                        panic!("Panic! $XDG_RUNTIME_DIR is set but is not accessible!");
                    }
                    Ok(stat_buf) =>
                    {
                        if stat_buf.perm.intersects(io::GROUP_RWX | io::OTHER_RWX)
                        {
                            panic!("Panic! $XDG_RUNTIME_DIR is insecure - should have permissions 0700!");
                        }
                    }
                }
            }
            None =>
            {
                if env("XDG_RUNTIME_DIR").is_some()
                {
                    panic!("Panic! $XDG_RUNTIME_DIR is set, but not absolute!");
                }
            }
        }

        XdgDirs
        {
            data_home: data_home,
            config_home: config_home,
            cache_home: cache_home,
            data_dirs: data_dirs,
            config_dirs: config_dirs,
            runtime_dir: runtime_dir,
        }
    }

    pub fn want_write_data<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_file(&self.data_home, b)
    }
    pub fn want_write_config<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_file(&self.config_home, b)
    }
    pub fn want_write_cache<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_file(&self.cache_home, b)
    }
    pub fn need_write_runtime<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_file(self.runtime_dir.as_ref().unwrap(), b)
    }
    pub fn want_mkdir_data<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_dir(&self.data_home, b)
    }
    pub fn want_mkdir_config<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_dir(&self.config_home, b)
    }
    pub fn want_mkdir_cache<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_dir(&self.cache_home, b)
    }
    pub fn need_mkdir_runtime<B: BytesContainer + Copy>(&self, b: B) -> Path
    {
        want_write_dir(self.runtime_dir.as_ref().unwrap(), b)
    }

    pub fn want_read_data<B: BytesContainer + Copy>(&self, b: B) -> Option<Path>
    {
        want_read_file(&self.data_home, &self.data_dirs, b)
    }
    pub fn want_read_config<B: BytesContainer + Copy>(&self, b: B) -> Option<Path>
    {
        want_read_file(&self.config_home, &self.config_dirs, b)
    }
    pub fn want_read_cache<B: BytesContainer + Copy>(&self, b: B) -> Option<Path>
    {
        want_read_file(&self.cache_home, &Vec::new(), b)
    }
    pub fn need_read_runtime<B: BytesContainer + Copy>(&self, b: B) -> Option<Path>
    {
        want_read_file(self.runtime_dir.as_ref().unwrap(), &Vec::new(), b)
    }

    pub fn want_list_data_all<B: BytesContainer + Copy>(&self, b: B) -> Vec<Path>
    {
        want_list_file_all(&self.data_home, &self.data_dirs, b)
    }
    pub fn want_list_config_all<B: BytesContainer + Copy>(&self, b: B) -> Vec<Path>
    {
        want_list_file_all(&self.config_home, &self.config_dirs, b)
    }
    pub fn want_list_data_once<B: BytesContainer + Copy>(&self, b: B) -> Vec<Path>
    {
        want_list_file_once(&self.data_home, &self.data_dirs, b)
    }
    pub fn want_list_config_once<B: BytesContainer + Copy>(&self, b: B) -> Vec<Path>
    {
        want_list_file_once(&self.config_home, &self.config_dirs, b)
    }
    pub fn want_list_cache<B: BytesContainer + Copy>(&self, b: B) -> Vec<Path>
    {
        want_list_file_all(&self.cache_home, &Vec::new(), b)
    }
    pub fn need_list_runtime<B: BytesContainer + Copy>(&self, b: B) -> Vec<Path>
    {
        want_list_file_all(self.runtime_dir.as_ref().unwrap(), &Vec::new(), b)
    }
}

fn getenv_one(env: |&str| -> Option<Vec<u8>>, var: &str) -> Option<Path>
{
    let val = env(var).unwrap_or(Vec::new());
    let path = Path::new(val);
    if !path.is_absolute()
    {
        None
    }
    else
    {
        Some(path)
    }
}

fn getenv_many(env: |&str| -> Option<Vec<u8>>, var: &str) -> Option<Vec<Path>>
{
    let val = env(var).unwrap_or(Vec::new());
    let paths = std::os::split_paths(val);
    let paths: Vec<Path> = paths.into_iter().filter(|p| p.is_absolute()).collect();
    if paths.is_empty()
    {
        None
    }
    else
    {
        Some(paths)
    }
}

fn want_write_file<B: BytesContainer + Copy>(home: &Path, b: B) -> Path
{
    let b = Path::new(b);
    let home = &home.join(b.dirname());
    let b = b.filename().unwrap();
    std::io::fs::mkdir_recursive(home, io::USER_RWX).unwrap();
    home.join(b)
}

fn want_write_dir<B: BytesContainer + Copy>(home: &Path, b: B) -> Path
{
    let joined = home.join(b);
    std::io::fs::mkdir_recursive(&joined, io::USER_RWX).unwrap();
    joined
}

fn want_read_file<B: BytesContainer + Copy>(home: &Path, dirs: &Vec<Path>, b: B) -> Option<Path>
{
    let p = home.join(b);
    if p.exists()
    {
        return Some(p);
    }
    for d in dirs.iter()
    {
        let p = d.join(b);
        if p.exists()
        {
            return Some(p);
        }
    }
    None
}

fn want_list_file_all<B: BytesContainer + Copy>(home: &Path, dirs: &Vec<Path>, b: B) -> Vec<Path>
{
    let mut vec = io::fs::readdir(&home.join(b)).unwrap_or(vec!());
    for path in dirs.iter()
    {
        vec.push_all(io::fs::readdir(&path.join(b)).unwrap_or(vec!()).as_slice());
    }
    return vec;
}

fn want_list_file_once<B: BytesContainer + Copy>(home: &Path, dirs: &Vec<Path>, b: B) -> Vec<Path>
{
    use std::collections::hashmap::HashSet;

    let mut vec = want_list_file_all(home, dirs, b);
    let mut seen = HashSet::<String>::new();
    vec = vec.into_iter().filter(|p| { let s = p.filename_str().to_string(); if seen.contains(&s) { return false; } seen.insert(s); return true; }).collect();
    return vec;
}

#[test]
fn test_files_exists()
{
    assert!(Path::new("test_files").exists());
    assert!(Path::new("test_files/runtime-bad").stat().unwrap().perm.intersects(io::GROUP_RWX | io::OTHER_RWX));
}

#[test]
fn test_bad_environment()
{
    use std::collections::hashmap::HashMap;

    let map: HashMap<String, String> =
        [
            ("XDG_DATA_HOME", "test_files/user/data"),
            ("XDG_CONFIG_HOME", "test_files/user/config"),
            ("XDG_CACHE_HOME", "test_files/user/cache"),
            ("XDG_DATA_DIRS", "test_files/user/data"),
            ("XDG_CONFIG_DIRS", "test_files/user/config"),
            // ("XDG_RUNTIME_DIR", "test_files/runtime-bad"),
        ].iter().map(|&(k, v)| (k.to_string(), v.to_string())).collect();
    let xd = XdgDirs::new_with_env(|v| map.find(&v.to_string()).map(|s| s.as_bytes().to_vec()));
    assert_eq!(xd.want_read_data("everywhere").map(|p| p.as_str().unwrap().to_string()), None);
    assert_eq!(xd.want_read_config("everywhere").map(|p| p.as_str().unwrap().to_string()), None);
    assert_eq!(xd.want_read_cache("everywhere").map(|p| p.as_str().unwrap().to_string()), None);
}

#[test]
fn test_good_environment()
{
    use std::collections::hashmap::HashMap;

    let cwd = std::os::make_absolute(&Path::new("."));
    let cwd = cwd.as_str().unwrap();

    let map: HashMap<String, String> =
        [
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
            //("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
        ].iter().map(|&(ref k, ref v)| (k.to_string(), v.clone())).collect();
    let xd = XdgDirs::new_with_env(|v| map.find(&v.to_string()).map(|s| s.as_bytes().to_vec()));
    assert!(xd.want_read_data("everywhere").map(|p| p.as_str().unwrap().to_string()) != None);
    assert!(xd.want_read_config("everywhere").map(|p| p.as_str().unwrap().to_string()) != None);
    assert!(xd.want_read_cache("everywhere").map(|p| p.as_str().unwrap().to_string()) != None);
}

#[test]
fn test_runtime_bad()
{
    let test_runtime_dir = std::os::make_absolute(&Path::new("test_files/runtime-bad"));
    let test_runtime_dir = test_runtime_dir.as_vec().to_vec();
    std::task::try(
        proc()
        {
            let _ = XdgDirs::new_with_env(|v| if v == "XDG_RUNTIME_DIR" { Some(test_runtime_dir.clone()) } else { None });
        }
    ).unwrap_err();
}

#[test]
fn test_runtime_good()
{
    use std::io::fs::File;

    let test_runtime_dir = std::os::make_absolute(&Path::new("test_files/runtime-good"));
    let _ = io::fs::rmdir_recursive(&test_runtime_dir);
    io::fs::mkdir_recursive(&test_runtime_dir, io::USER_RWX).unwrap();
    let test_runtime_dir = test_runtime_dir.as_vec().to_vec();
    let xd = XdgDirs::new_with_env(|v| if v == "XDG_RUNTIME_DIR" { Some(test_runtime_dir.clone()) } else { None });
    xd.need_mkdir_runtime("foo");
    assert!(Path::new("test_files/runtime-good/foo").is_dir());
    let w = xd.need_write_runtime("bar/baz");
    assert!(Path::new("test_files/runtime-good/bar").is_dir());
    assert!(!Path::new("test_files/runtime-good/bar/baz").exists());
    File::create(&w).unwrap();
    assert!(Path::new("test_files/runtime-good/bar/baz").exists());
    assert!(xd.need_read_runtime("bar/baz") == Some(w.clone()));
    File::open(&w).unwrap();
    io::fs::unlink(&w).unwrap();
    let root: Vec<Path> = xd.need_list_runtime(".");
    let mut root: Vec<String> = root.into_iter().map(|p| make_relative(&p).as_str().unwrap().to_string()).collect();
    root.sort();
    assert_eq!(root, vec!["test_files/runtime-good/bar".to_string(), "test_files/runtime-good/foo".to_string()]);
    assert!(xd.need_list_runtime("bar").is_empty());
    assert!(xd.need_read_runtime("foo/qux").is_none());
    assert!(xd.need_read_runtime("qux/foo").is_none());
    assert!(!Path::new("test_files/runtime-good/qux").exists());
}

#[test]
fn test_lists()
{
    use std::collections::hashmap::HashMap;

    let cwd = std::os::make_absolute(&Path::new("."));
    let cwd = cwd.as_str().unwrap();

    let map: HashMap<String, String> =
        [
            ("XDG_DATA_HOME", format!("{}/test_files/user/data", cwd)),
            ("XDG_CONFIG_HOME", format!("{}/test_files/user/config", cwd)),
            ("XDG_CACHE_HOME", format!("{}/test_files/user/cache", cwd)),
            ("XDG_DATA_DIRS", format!("{}/test_files/system0/data:{}/test_files/system1/data:{}/test_files/system2/data:{}/test_files/system3/data", cwd, cwd, cwd, cwd)),
            ("XDG_CONFIG_DIRS", format!("{}/test_files/system0/config:{}/test_files/system1/config:{}/test_files/system2/config:{}/test_files/system3/config", cwd, cwd, cwd, cwd)),
            //("XDG_RUNTIME_DIR", format!("{}/test_files/runtime-bad", cwd)),
        ].iter().map(|&(ref k, ref v)| (k.to_string(), v.clone())).collect();
    let xd = XdgDirs::new_with_env(|v| map.find(&v.to_string()).map(|s| s.as_bytes().to_vec()));

    let files: Vec<Path> = xd.want_list_config_all(".");
    let mut files: Vec<String> = files.into_iter().map(|p| make_relative(&p).as_str().unwrap().to_string()).collect();
    files.sort();
    let files = files;
    assert_eq!(files, [
               "test_files/system1/config/both_system_config.file",
               "test_files/system1/config/everywhere",
               "test_files/system1/config/system1_config.file",
               "test_files/system2/config/both_system_config.file",
               "test_files/system2/config/everywhere",
               "test_files/system2/config/system2_config.file",
               "test_files/user/config/everywhere",
               "test_files/user/config/user_config.file",
    ].iter().map(|s| s.to_string()).collect());

    let files: Vec<Path> = xd.want_list_config_once(".");
    let mut files: Vec<String> = files.into_iter().map(|p| make_relative(&p).as_str().unwrap().to_string()).collect();
    files.sort();
    let files = files;
    assert_eq!(files, [
               "test_files/system1/config/both_system_config.file",
               "test_files/system1/config/system1_config.file",
               "test_files/system2/config/system2_config.file",
               "test_files/user/config/everywhere",
               "test_files/user/config/user_config.file",
    ].iter().map(|s| s.to_string()).collect());
}

#[cfg(test)]
fn make_relative(p: &Path) -> Path
{
    let cwd = Path::new(".");
    let cwd = std::os::make_absolute(&cwd);
    p.path_relative_from(&cwd).unwrap()
}
