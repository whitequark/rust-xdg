//! # desktop_entry
//! Implementation of the [XDG Desktop Entry Specification][xdg-desktop-entry].
//!
//! [xdg-desktop-entry]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
//!

#[warn(missing_docs)]

use regex::Regex;
use ini::Ini;
use std::collections::HashMap;

type LocaleString = String;
type IconString = String;
type Strings = Vec<String>;
type LocaleStrings = Vec<LocaleString>;

const DEFAULT_GROUP: &str = "Desktop Entry";

/// DektopFile allows to load and validate desktop files according
/// to the [X Desktop Group Desktop File Entry specification][xdg-desktop-file].
///
/// # Examples
///
/// To load a desktop file `foo.desktop`:
///
/// ```
/// use xdg::desktop_entry::DesktopFile;
///
/// let desktop_file = DesktopFile::from_file("foo.desktop").unwrap();
/// assert_eq!(desktop_file.name, "Foo")
/// ```
///
/// To validate the desktop file:
///
/// ```
/// let result: Result<(), String> = desktop_file.validate();
/// assert_eq!(result, Ok(()))
/// ```
///
/// To get the default group use the `get_default_group` method.
///
/// [xdg-desktop-file]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
///
pub struct DesktopFile {
    pub filename: String,
    pub groups: Vec<DesktopEntry>
}

/// Individual group header for a desktop file.
/// The struc fields correspond to the possible
/// [recognized desktop entry keys][xdg-keys], with the
/// exception of Type which is replaced with `type_string`.
///
/// [xdg-keys]: https://specifications.freedesktop.org/desktop-entry-spec/latest/ar01s06.html
///
#[derive(Debug, Clone)]
pub struct DesktopEntry {
    pub entry_type: String,
    pub type_string: Option<String>, // type is a reserver keyword
    pub version: Option<String>,
    pub name: Option<LocaleString>, // Required
    pub generic_name: Option<LocaleString>,
    pub no_display: Option<bool>,
    pub comment: Option<LocaleString>,
    pub icon: Option<IconString>,
    pub hidden: Option<bool>,
    pub only_show_in: Option<Strings>,
    pub not_show_in: Option<Strings>,
    pub dbus_activatable: Option<bool>,
    pub try_exec: Option<String>,
    pub exec: Option<String>,
    pub path: Option<String>,
    pub terminal: Option<bool>,
    pub actions: Option<String>,
    pub mime_type: Option<Strings>,
    pub categories: Option<Strings>,
    pub implements: Option<Strings>,
    pub keywords: Option<LocaleStrings>,
    pub startup_notify: Option<bool>,
    pub startup_wm_class: Option<String>,
    pub url: Option<String>, // Required for Link type entries
    pub prefers_non_default_gpu: Option<bool>,
}

impl DesktopEntry {
    fn from_hash_map(section: String, hash: &HashMap<String, String>) -> Self {
        use std::str::FromStr;
        fn convert_str_strings(s: &str) -> Strings {
            s.split(";").map(|x| x.to_string()).filter(|x| x.len() > 0 ).collect::<Vec<String>>()
        }
        let type_string = hash.get("Type").map(|x| x.to_string());
        let version = hash.get("Version").map(|x| x.to_string());
        let name = hash.get("Name").map(|x| x.to_string());
        let generic_name = hash.get("GenericName").map(|x| x.to_string());
        let no_display = hash.get("NoDisplay").map(|x| FromStr::from_str(x).ok()).flatten();
        let comment = hash.get("Comment").map(|x| x.to_string());
        let icon = hash.get("Icon").map(|x| x.to_string());
        let hidden = hash.get("Hidden").map(|x| FromStr::from_str(x).ok()).flatten();
        let only_show_in = hash.get("OnlyShowIn").map(|x| convert_str_strings(x));
        let not_show_in = hash.get("NotShowIn").map(|x| convert_str_strings(x));
        let dbus_activatable = hash.get("DBusActivatable").map(|x| FromStr::from_str(x).ok()).flatten();
        let try_exec = hash.get("TryExec").map(|x| x.to_string());
        let exec = hash.get("Exec").map(|x| x.to_string());
        let path = hash.get("Path").map(|x| x.to_string());
        let terminal = hash.get("Terminal").map(|x| FromStr::from_str(x).ok()).flatten();
        let actions = hash.get("Actions").map(|x| x.to_string());
        let mime_type = hash.get("MimeType").map(|x| convert_str_strings(x));
        let categories = hash.get("Categories").map(|x| convert_str_strings(x));
        let implements = hash.get("Implements").map(|x| convert_str_strings(x));
        let keywords = hash.get("Keywords").map(|x| convert_str_strings(x));
        let startup_notify = hash.get("StartupNotify").map(|x| FromStr::from_str(x).ok()).flatten();
        let startup_wm_class = hash.get("StartupWMClass").map(|x| x.to_string());
        let url = hash.get("URL").map(|x| x.to_string());
        let prefers_non_default_gpu = hash.get("PrefersNonDefaultGPU").map(|x| FromStr::from_str(x).ok()).flatten();
        let desktop_entry = Self {
            entry_type: section,
            type_string,
            version,
            name,
            generic_name,
            no_display,
            comment,
            icon,
            hidden,
            only_show_in,
            not_show_in,
            dbus_activatable,
            try_exec,
            exec,
            path,
            terminal,
            actions,
            mime_type,
            categories,
            implements,
            keywords,
            startup_notify,
            startup_wm_class,
            url,
            prefers_non_default_gpu,
        };
        desktop_entry
    }

    fn check_not_show_in(&self) -> Result<(), String> {
        let mut warning = String::new();
        if let Some(items) = &self.not_show_in {
            let valid = ["GNOME", "KDE", "LXDE", "MATE", "Razor", "ROX", "TDE", "Unity",
                         "XFCE", "Old"];
            for item in items {
                let starts_with = item.starts_with("X-");
                if !valid.contains(&item.as_str()) && !starts_with {
                    warning += &format!("'{}' is not a registered OnlyShowIn value", item);
                }
            }
            if warning.len() > 0 {
                return Err(warning)
            } else {
                return Ok(())
            }
        }
        Ok(())
    }

    fn check_only_show_in(&self) -> Result<(), String> {
        let mut warning = String::new();
        if let Some(items) = &self.only_show_in {
            let valid = ["GNOME", "KDE", "LXDE", "MATE", "Razor", "ROX", "TDE", "Unity",
                         "XFCE", "Old"];
            for item in items {
                let starts_with = item.starts_with("X-");
                if !valid.contains(&item.as_str()) && !starts_with {
                    warning += &format!("'{}' is not a registered OnlyShowIn value", item);
                }
            }
            if warning.len() > 0 {
                return Err(warning)
            } else {
                return Ok(())
            }
        }
        Ok(())
    }

    fn check_try_exec(&self) -> Result<(), String> {
        if let Some(try_exec) = &self.try_exec {
            let err = "Could not find".to_string() + &try_exec;
            return which::which(try_exec).and(Ok(())).or(Err(err))
        }
        Ok(())
    }

    fn check_group(&self) -> Result<(), String>{
        let re1 = Regex::new(r"^Desktop Action [a-zA-Z0-9-]+$").unwrap();
        let re2 = Regex::new(r"^X-").unwrap();
        let group: &str = &self.entry_type;
        let mut err = String::new();
        if ! (group == DEFAULT_GROUP || re1.is_match(group) || re2.is_match(group) && group.is_ascii()) {
            err += "Invalid Group name: ";
            err += group;
        } else if self.only_show_in.is_some() && self.not_show_in.is_some() {
            err += "Group may either have OnlyShowIn or NotShowIn, but not both";
        }
        if err.len() > 0 {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn check_extras(&self) -> Result<(), String>{
        let group = &self.entry_type;
        let mut err = String::new();

        if group == "KDE Desktop Entry" {
            err += "[KDE Desktop Entry] Header is deprecated";
        }
        if self.type_string.is_none() {
            err += "Key 'Type' is missing";
        }
        if self.name.is_none() {
            err += "Key 'Name' is missing";
        }

        if err.len() > 0 {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn check_keys(&self, filename: &str) -> Result<(), String> {
        use std::ffi::OsStr;
        use std::path::Path;

        let mut warnings = String::new();
        let file_ext = Path::new(filename)
            .extension()
            .and_then(OsStr::to_str).unwrap();
        if let Some(etype) = &self.type_string {
            if etype == "ServiceType" || etype == "Service" || etype == "FSDevice" {
                warnings += &format!("Type={} is a KDE extension", etype);
            } else if etype == "MimeType" {
                warnings += "Type=MimeType is deprecated";
            } else if !(etype == "Application" || etype == "Link" || etype == "Directory") {
                warnings += &format!("Value of key 'Type' must be Application, Link or Directory, but is {}", etype)
            };

            if file_ext == ".directory" && !(etype == "Directory") {
                warnings += &format!("File extension is .directory, but Type is {}", etype);
            } else if file_ext == ".desktop" && etype == "Directory" {
                warnings += "Files with Type=Directory should have the extension .directory";
            }
            if etype == "Application" {
                if self.exec.is_none() {
                    warnings += "Type=Application needs 'Exec' key";
                }
            }
            if etype == "Link" {
                if self.url.is_none() {
                    warnings += "Type=Link needs 'URL' key";
                }
            }
        }

        if let Some(_) = &self.only_show_in {
            self.check_only_show_in()?;
        }

        if let Some(_) = &self.not_show_in {
            self.check_not_show_in()?;
        }

        if warnings.len() > 0 {
            Err(warnings)
        } else {
            Ok(())
        }
    }

    /// Validates the group, the error `Err(error)` contains the warnings.
    pub fn validate(&self) -> Result<(), String> {
        todo!();
    }
}

impl DesktopFile {
    fn load_ini(ini: &str) -> Vec<(String, HashMap<String, String>)> {
        let i = Ini::load_from_file(ini).unwrap();
        let mut result = vec!();
        for (sec, prop) in i.iter() {
            let mut s = HashMap::new();
            for (k, v) in prop.iter() {
                s.insert(k.to_string(), v.to_string());
            }
            result.push((sec.unwrap().to_string(), s));
        }
        result
    }

    fn from_hash_map(hash: &Vec<(String, HashMap<String, String>)>, filename: &str) -> Self {
        let mut groups = vec!();
        for (entry_name, entry) in hash.iter() {
            groups.push(DesktopEntry::from_hash_map(entry_name.into(), entry));
        }
        let desktop_file = Self {
            filename: filename.into(),
            groups,
        };
        desktop_file
    }

    /// Load a `DesktopFile` from a file `filename`.
    pub fn from_file(filename: &str) -> Option<Self> {
        let hash = Self::load_ini(filename);
        let desktop_file = Self::from_hash_map(&hash, filename);
        // TODO this should not load if there is any issue.
        Some(desktop_file)
    }

    fn check_extension(&self) -> Result<(), String> {
        use std::path::Path;
        use std::ffi::OsStr;

        let mut err = String::new();
        let extension = Path::new(&self.filename)
            .extension()
            .and_then(OsStr::to_str).unwrap();
        match extension {
            ".desktop" => (),
            ".directory" => (),
            ".kdelnk" => {
                err += "File extension .kdelnk is deprecated";
            },
            _ => {
                err += "Unknown File extension";
            },
        };

        Ok(())
    }

    /// Get the group with header "Desktop Entry"
    pub fn get_default_group(&self) -> Option<DesktopEntry> {
        // TODO Improve this function
        Some(self.groups[0].clone())
    }

    pub fn validate(&self) -> Result<(), String> {
        // TODO Improve errors
        let default_group = &self.get_default_group().unwrap();
        self.check_extension()?;
        default_group.check_keys(&self.filename)?;
        for group in &self.groups {
            group.check_group()?;
            group.check_extras()?;
            group.check_try_exec()?;
        }
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use crate::desktop_entry::DesktopFile;
    #[test]
    fn parse_desktop_file() {
        let filename ="test_files/desktop_entries/test-multiple.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let groups = desktop_file.groups;
        assert_eq!(desktop_file.filename, filename);
        assert_eq!(groups.len(), 2);
    }
    #[test]
    fn parse_groups() {
        use crate::desktop_entry::DEFAULT_GROUP;
        let filename ="test_files/desktop_entries/test-multiple.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let groups = desktop_file.groups;
        let g1 = groups.get(0).unwrap();
        let g2 = groups.get(1).unwrap();
        assert_eq!(g1.entry_type, DEFAULT_GROUP);
        assert_eq!(g2.entry_type, "Desktop Action new-empty-window");
        assert_eq!(g1.categories.as_ref().unwrap().len(), 4)
    }

    #[test]
    fn try_exec() {
        let filename ="test_files/desktop_entries/test-multiple.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let default_group = &desktop_file.groups[0];
        let result = default_group.check_try_exec();
        let sec_group = &desktop_file.groups[1];
        let result2 = sec_group.check_try_exec().is_err();
        assert_eq!(result, Ok(()));
        assert_eq!(result2, false);
    }

    #[test]
    fn check_group() {
        let filename ="test_files/desktop_entries/test-multiple.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let groups = desktop_file.groups;
        let default_group = groups.get(0).unwrap();
        assert_eq!(default_group.check_group(), Ok(()));
        let filename ="test_files/desktop_entries/fail.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let groups = desktop_file.groups;
        let default_group = groups.get(0).unwrap();
        assert_eq!(default_group.check_group().is_err(), true);
    }
}
