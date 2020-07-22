//! # desktop_entry
//! Implementation of the [XDG Desktop Entry Specification][xdg-desktop-entry].
//!
//! [xdg-desktop-entry]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
//!

// TODO Add custom X- groups support
use ini::Ini;
use regex::Regex;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;

type IconString = String;
type Strings = Vec<String>;

#[derive(Clone)]
enum LocaleLang {
    Default,
    Lang(String),
}

#[derive(Clone)]
pub struct Locale {
    lang: LocaleLang,
    value: String,
}

#[derive(Clone)]
pub struct Locales {
    lang: LocaleLang,
    values: Strings,
}

type LocaleString = Vec<Locale>;
type LocaleStrings = Vec<Locales>;

impl TryFrom<Locales> for Locale {
    type Error = Error;
    fn try_from(locales: Locales) -> Result<Self> {
        if locales.values.is_empty() {
            Err(Error::from("Could not conver locales to locale"))
        } else {
            Ok(Self {
                value: locales.values[0].clone(),
                lang: locales.lang,
            })
        }
    }
}

const DEFAULT_GROUP: &str = "Desktop Entry";

/// This type allows to load and validate desktop files according
/// to the [X Desktop Group Desktop File Entry specification][xdg-desktop-file].
///
/// # Examples
///
/// To load a desktop file `foo.desktop`:
///
/// ```
/// use xdg::desktop_entry::DesktopFile;
///
/// let desktop_file = DesktopFile::from_file("test_files/desktop_entries/test.desktop").unwrap();
/// let name = desktop_file.get_name().ok();
///
/// assert_eq!(name, Some("Foo".to_string()));
/// ```
///
/// To validate the desktop file:
///
/// ```
/// use xdg::desktop_entry::DesktopFile;
///
/// let desktop_entry = "[Desktop Entry]\nType=Application\nName=Foo\nExec=Bar";
///
/// let desktop_entry_file = DesktopFile::from_str(desktop_entry).unwrap();
/// let result = desktop_entry_file.validate();
/// assert_eq!(result.is_ok(), true);
/// ```
///
/// To get the default group use the `get_default_group` method.
///
/// [xdg-desktop-file]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
///
pub struct DesktopFile {
    pub filename: String,
    pub groups: Vec<DesktopEntry>,
}

/// Individual group header for a desktop file.
/// The struc fields correspond to the possible
/// [recognized desktop entry keys][xdg-keys], with the
/// exception of Type which is replaced with `type_string`.
///
/// [xdg-keys]: https://specifications.freedesktop.org/desktop-entry-spec/latest/ar01s06.html
///
#[derive(Clone)]
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
    pub actions: Option<Strings>,
    pub mime_type: Option<Strings>,
    pub categories: Option<Strings>,
    pub implements: Option<Strings>,
    pub keywords: Option<LocaleStrings>,
    pub startup_notify: Option<bool>,
    pub startup_wm_class: Option<String>,
    pub url: Option<String>, // Required for Link type entries
    pub prefers_non_default_gpu: Option<bool>,
}

// TODO Find a better type
#[derive(Debug)]
pub struct Error(Vec<String>);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let message = self.0.join(" ");
        write!(f, "{}", message)
    }
}

impl From<&str> for Error {
    fn from(error: &str) -> Self {
        Error::from(error.to_string())
    }
}

impl From<String> for Error {
    fn from(error: String) -> Self {
        Error(vec![error])
    }
}
impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

impl DesktopEntry {
    fn from_hash_map(section: String, hashmap: &HashMap<String, String>) -> Result<Self> {
        use std::str::FromStr;

        let type_string = hashmap.get("Type").map(|x| x.to_string());
        let version = hashmap.get("Version").map(|x| x.to_string());
        let name = locale_string_from_hashmap("Name", hashmap);
        let generic_name = locale_string_from_hashmap("GenericName", hashmap);
        let no_display = hashmap
            .get("NoDisplay")
            .map(|x| FromStr::from_str(x).ok())
            .flatten();
        let comment = locale_string_from_hashmap("Comment", hashmap);
        let icon = hashmap.get("Icon").map(|x| x.to_string());
        let hidden = hashmap
            .get("Hidden")
            .map(|x| FromStr::from_str(x).ok())
            .flatten();
        let only_show_in = hashmap.get("OnlyShowIn").map(|x| parse_strings(x));
        let not_show_in = hashmap.get("NotShowIn").map(|x| parse_strings(x));
        let dbus_activatable = hashmap
            .get("DBusActivatable")
            .map(|x| FromStr::from_str(x).ok())
            .flatten();
        let try_exec = hashmap.get("TryExec").map(|x| x.to_string());
        let exec = hashmap.get("Exec").map(|x| x.to_string());
        let path = hashmap.get("Path").map(|x| x.to_string());
        let terminal = hashmap
            .get("Terminal")
            .map(|x| FromStr::from_str(x).ok())
            .flatten();
        let actions = hashmap.get("Actions").map(|x| parse_strings(x));
        let mime_type = hashmap.get("MimeType").map(|x| parse_strings(x));
        let categories = hashmap.get("Categories").map(|x| parse_strings(x));
        let implements = hashmap.get("Implements").map(|x| parse_strings(x));
        let keywords = locale_strings_from_hashmap("Keywords", hashmap);
        let startup_notify = hashmap
            .get("StartupNotify")
            .map(|x| FromStr::from_str(x).ok())
            .flatten();
        let startup_wm_class = hashmap.get("StartupWMClass").map(|x| x.to_string());
        let url = hashmap.get("URL").map(|x| x.to_string());
        let prefers_non_default_gpu = hashmap
            .get("PrefersNonDefaultGPU")
            .map(|x| FromStr::from_str(x).ok())
            .flatten();
        let desktop_entry = DesktopEntry {
            entry_type: section.to_string(),
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
        desktop_entry.validate()?;
        Ok(desktop_entry)
    }

    fn check_not_show_in(&self) -> Result<()> {
        let mut warning: Vec<String> = vec![];
        if let Some(items) = &self.not_show_in {
            let valid = [
                "GNOME", "KDE", "LXDE", "MATE", "Razor", "ROX", "TDE", "Unity", "XFCE", "Old",
            ];
            for item in items {
                let starts_with = item.starts_with("X-");
                if !valid.contains(&item.as_str()) && !starts_with {
                    warning.push(format!("'{}' is not a registered OnlyShowIn value", item));
                }
            }
            if warning.len() > 0 {
                return Err(Error(warning));
            } else {
                return Ok(());
            }
        }
        Ok(())
    }

    fn check_only_show_in(&self) -> Result<()> {
        let mut warning: Strings = vec![];
        if let Some(items) = &self.only_show_in {
            let valid = [
                "GNOME", "KDE", "LXDE", "MATE", "Razor", "ROX", "TDE", "Unity", "XFCE", "Old",
            ];
            for item in items {
                let starts_with = item.starts_with("X-");
                if !valid.contains(&item.as_str()) && !starts_with {
                    warning.push(format!("'{}' is not a registered OnlyShowIn value", item));
                }
            }
            if warning.len() > 0 {
                return Err(Error(warning));
            } else {
                return Ok(());
            }
        }
        Ok(())
    }

    fn check_try_exec(&self) -> Result<()> {
        if let Some(try_exec) = &self.try_exec {
            let err: Strings = vec![format!("Could not find {}", try_exec)];
            return which::which(try_exec).and(Ok(())).or(Err(Error(err)));
        }
        Ok(())
    }

    fn check_group(&self) -> Result<()> {
        let re1 = Regex::new(r"^Desktop Action [a-zA-Z0-9-]+$").unwrap();
        let re2 = Regex::new(r"^X-").unwrap();
        let group: &str = &self.entry_type;
        let mut err: Vec<String> = vec![];
        if !(group == DEFAULT_GROUP
            || re1.is_match(group)
            || re2.is_match(group) && group.is_ascii())
        {
            err.push(format!("Invalid Group name: {}", group));
        } else if self.only_show_in.is_some() && self.not_show_in.is_some() {
            err.push("Group may either have OnlyShowIn or NotShowIn, but not both".to_string());
        }
        if err.len() > 0 {
            Err(Error(err))
        } else {
            Ok(())
        }
    }

    fn check_categories(&self) -> Result<()> {
        let main = [
            "AudioVideo",
            "Audio",
            "Video",
            "Development",
            "Education",
            "Game",
            "Graphics",
            "Network",
            "Office",
            "Science",
            "Settings",
            "System",
            "Utility",
        ];
        let additional = [
            "Building",
            "Debugger",
            "IDE",
            "GUIDesigner",
            "Profiling",
            "RevisionControl",
            "Translation",
            "Calendar",
            "ContactManagement",
            "Database",
            "Dictionary",
            "Chart",
            "Email",
            "Finance",
            "FlowChart",
            "PDA",
            "ProjectManagement",
            "Presentation",
            "Spreadsheet",
            "WordProcessor",
            "2DGraphics",
            "VectorGraphics",
            "RasterGraphics",
            "3DGraphics",
            "Scanning",
            "OCR",
            "Photography",
            "Publishing",
            "Viewer",
            "TextTools",
            "DesktopSettings",
            "HardwareSettings",
            "Printing",
            "PackageManager",
            "Dialup",
            "InstantMessaging",
            "Chat",
            "IRCClient",
            "Feed",
            "FileTransfer",
            "HamRadio",
            "News",
            "P2P",
            "RemoteAccess",
            "Telephony",
            "TelephonyTools",
            "VideoConference",
            "WebBrowser",
            "WebDevelopment",
            "Midi",
            "Mixer",
            "Sequencer",
            "Tuner",
            "TV",
            "AudioVideoEditing",
            "Player",
            "Recorder",
            "DiscBurning",
            "ActionGame",
            "AdventureGame",
            "ArcadeGame",
            "BoardGame",
            "BlocksGame",
            "CardGame",
            "KidsGame",
            "LogicGame",
            "RolePlaying",
            "Shooter",
            "Simulation",
            "SportsGame",
            "StrategyGame",
            "Art",
            "Construction",
            "Music",
            "Languages",
            "ArtificialIntelligence",
            "Astronomy",
            "Biology",
            "Chemistry",
            "ComputerScience",
            "DataVisualization",
            "Economy",
            "Electricity",
            "Geography",
            "Geology",
            "Geoscience",
            "History",
            "Humanities",
            "ImageProcessing",
            "Literature",
            "Maps",
            "Math",
            "NumericalAnalysis",
            "MedicalSoftware",
            "Physics",
            "Robotics",
            "Spirituality",
            "Sports",
            "ParallelComputing",
            "Amusement",
            "Archiving",
            "Compression",
            "Electronics",
            "Emulator",
            "Engineering",
            "FileTools",
            "FileManager",
            "TerminalEmulator",
            "Filesystem",
            "Monitor",
            "Security",
            "Accessibility",
            "Calculator",
            "Clock",
            "TextEditor",
            "Documentation",
            "Adult",
            "Core",
            "KDE",
            "GNOME",
            "XFCE",
            "GTK",
            "Qt",
            "Motif",
            "Java",
            "ConsoleOnly",
        ];
        if let Some(categories) = &self.categories {
            let n_main_categories = categories.iter().filter(|x| main.contains(&x.as_str()));
            if n_main_categories.count() == 0 {
                return Err(Error::from("Missing main category"));
            }
            let invalid_categories = categories.iter().filter(|x| {
                !x.starts_with("X-")
                    && !main.contains(&x.as_str())
                    && !additional.contains(&x.as_str())
            });
            let x: Vec<String> = invalid_categories
                .map(|x| format!("{} is not a registered Category", x))
                .collect();
            if x.len() > 0 {
                return Err(Error(x));
            }
        }
        Ok(())
    }

    fn is_default_grop(&self) -> bool {
        // TODO verify there are no more cases
        let group: &str = &self.entry_type;
        if group == DEFAULT_GROUP {
            true
        } else {
            false
        }
    }

    fn check_extras(&self) -> Result<()> {
        let group = &self.entry_type;
        let mut err: Strings = vec![];

        if group == "KDE Desktop Entry" {
            err.push("[KDE Desktop Entry] Header is deprecated".to_string());
        }
        if self.type_string.is_none() && self.is_default_grop() {
            err.push("Key 'Type' is missing".to_string());
        }
        if self.name.is_none() {
            err.push("Key 'Name' is missing".to_string());
        }

        if err.len() > 0 {
            Err(Error(err))
        } else {
            Ok(())
        }
    }

    fn check_keys(&self) -> Result<()> {
        let mut warnings: Strings = vec![];
        if let Some(etype) = &self.type_string {
            if etype == "ServiceType" || etype == "Service" || etype == "FSDevice" {
                warnings.push(format!("Type={} is a KDE extension", etype));
            } else if etype == "MimeType" {
                warnings.push("Type=MimeType is deprecated".to_string());
            } else if !(etype == "Application" || etype == "Link" || etype == "Directory") {
                warnings.push(format!(
                    "Value of key 'Type' must be Application, Link or Directory, but is {}",
                    etype
                ))
            };

            if etype == "Application" {
                if self.exec.is_none() {
                    warnings.push("Type=Application needs 'Exec' key".to_string());
                }
            }
            if etype == "Link" {
                if self.url.is_none() {
                    warnings.push("Type=Link needs 'URL' key".to_string());
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
            Err(Error(warnings))
        } else {
            Ok(())
        }
    }

    /// Validates the group, the error `Err(error)` contains the warnings.
    pub fn validate(&self) -> Result<()> {
        &self.check_keys()?;
        &self.check_group()?;
        &self.check_extras()?;
        &self.check_try_exec()?;
        &self.check_not_show_in()?;
        &self.check_only_show_in()?;
        &self.check_categories()?;
        Ok(())
    }
}

impl fmt::Display for DesktopFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut string = String::new();
        for group in &self.groups {
            string += &group.to_string();
        }
        write!(f, "{}", string)
    }
}

impl fmt::Display for DesktopEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut string = format!("[{}]", self.entry_type);
        let mut append_string = |opt: &Option<String>, key: &str| {
            if let Some(s) = opt {
                string += "\n";
                string += key;
                string += "=";
                string += &s;
            };
        };
        append_string(&self.type_string, "Type");
        append_string(&self.version, "Version");
        append_string(&self.exec, "Exec");
        append_string(&self.path, "Path");
        append_string(&self.startup_wm_class, "StartupWMClass");
        append_string(&self.url, "Url");
        append_string(&self.path, "Path");
        append_string(&self.try_exec, "TryExec");

        // Icon strings
        append_string(&self.icon, "Icon");

        // Locale strings
        let mut append_string = |opt: &Option<LocaleString>, key: &str| {
            if let Some(locale_string) = opt {
                for locale in locale_string.iter() {
                    let value = locale.value.clone();
                    match &locale.lang {
                        LocaleLang::Lang(lang) => {
                            string += &format!("\n{}[{}]={}", key, lang, value)
                        }
                        _ => string += &format!("\n{}={}", key, value),
                    }
                }
            };
        };
        append_string(&self.name, "Name");
        append_string(&self.generic_name, "GenericName");
        append_string(&self.comment, "Comment");

        let mut append_bool = |opt: &Option<bool>, key: &str| {
            if let Some(s) = opt {
                string += "\n";
                string += key;
                string += "=";
                string += &s.to_string();
            };
        };
        append_bool(&self.no_display, "NoDisplay");
        append_bool(&self.hidden, "Hidden");
        append_bool(&self.dbus_activatable, "DBusActivatable");
        append_bool(&self.startup_notify, "StartupNotify");
        append_bool(&self.prefers_non_default_gpu, "PrefersNonDefaultGPU");
        append_bool(&self.no_display, "NoDisplay");

        let mut append_strings = |opt: &Option<Strings>, key: &str| {
            if let Some(s) = opt {
                let values = s.join(";");
                string += "\n";
                string += key;
                string += "=";
                string += &values;
                string += ";";
            };
        };

        append_strings(&self.only_show_in, "OnlyShowIn");
        append_strings(&self.actions, "Actions");
        append_strings(&self.not_show_in, "NotShowIn");
        append_strings(&self.mime_type, "MimeType");
        append_strings(&self.categories, "Categories");
        append_strings(&self.implements, "Implements");

        // Locale strings.
        let mut append_strings = |opt: &Option<LocaleStrings>, key: &str| {
            if let Some(locale_strings) = opt {
                for locale in locale_strings.iter() {
                    let values = locale.values.join(";");
                    match &locale.lang {
                        LocaleLang::Lang(lang) => {
                            string += &format!("\n{}[{}]={};", key, lang, values)
                        }
                        _ => string += &format!("\n{}={}", key, values),
                    }
                }
            };
        };
        append_strings(&self.keywords, "Keywords");

        write!(f, "{}", string)
    }
}

/// Writes the contents of a `DesktopFile` to a file `filename`.
///
/// ```
/// use xdg::desktop_entry::DesktopFile;
/// use std::fs::File;
/// use std::io::prelude::*;
/// use std::error::Error;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let desktop_entry_contents = "[Desktop Entry]\nType=Application\nExec=Bar\nName=Foo";
///     let d_entry = DesktopFile::from_str(desktop_entry_contents)?;
///     d_entry.to_file("foo.desktop")?;
///
///     let mut file = File::open("foo.desktop")?;
///     let mut s = String::new();
///     let contents = file.read_to_string(&mut s)?;
///     // Note that the order of the lines in the generated file is deterministic,
///     // and could not coincide with the original file.
///     assert_eq!(s, desktop_entry_contents.to_string());
///     Ok(())
/// }
/// ```
impl DesktopFile {
    pub fn to_file(&self, filename: &str) -> Result<()> {
        use std::fs::File;
        use std::io::prelude::*;

        let mut file = File::create(filename).unwrap();
        if let Ok(_) = file.write_all(self.to_string().as_bytes()) {
            Ok(())
        } else {
            Err(Error::from("Could not write"))
        }
    }

    pub fn get_name(&self) -> Result<String> {
        let err = Error(vec!["Could not read default group".to_string()]);
        let err2 = Error(vec!["Could not read name".to_string()]);
        get_default_value(self.get_default_group().ok_or(err)?.name.ok_or(err2)?)
    }

    fn load_ini(ini: &str) -> Vec<(String, HashMap<String, String>)> {
        let i = Ini::load_from_file(ini).unwrap();
        let mut result = vec![];
        for (sec, prop) in i.iter() {
            let mut s = HashMap::new();
            for (k, v) in prop.iter() {
                s.insert(k.to_string(), v.to_string());
            }
            result.push((sec.unwrap().to_string(), s));
        }
        result
    }

    /// Loads a desktop entry from a string.
    ///
    /// # Example
    ///
    /// ```
    /// use xdg::desktop_entry::DesktopFile;
    ///
    /// let desktop_entry = "[Desktop Entry]\nType=Application\nName=Foo\nExec=Bar";
    /// let loaded_entry = DesktopFile::from_str(desktop_entry).unwrap();
    /// assert_eq!(loaded_entry.get_name().unwrap(), "Foo".to_string());
    /// ```
    pub fn from_str(input: &str) -> Result<Self> {
        let i = Ini::load_from_str(input).unwrap();

        let mut result = vec![];
        for (sec, prop) in i.iter() {
            let mut s = HashMap::new();
            for (k, v) in prop.iter() {
                s.insert(k.to_string(), v.to_string());
            }
            result.push((sec.unwrap().to_string(), s));
        }
        let desktop_file = Self::from_hash_map(&result, "str.desktop")?;
        Ok(desktop_file)
    }

    fn from_hash_map(
        hash: &Vec<(String, HashMap<String, String>)>,
        filename: &str,
    ) -> Result<Self> {
        let mut groups = vec![];
        for (entry_name, entry) in hash.iter() {
            groups.push(DesktopEntry::from_hash_map(entry_name.into(), &entry)?);
        }
        let desktop_file = Self {
            filename: filename.into(),
            groups,
        };
        desktop_file.check_extension()?;
        desktop_file.validate()?;
        Ok(desktop_file)
    }

    /// Load a `DesktopFile` from a file `filename`.
    pub fn from_file(filename: &str) -> Result<Self> {
        let hash = Self::load_ini(filename);
        let desktop_file = Self::from_hash_map(&hash, filename);
        desktop_file
    }

    fn check_extension(&self) -> Result<()> {
        use std::ffi::OsStr;
        use std::path::Path;

        let mut err = String::new();
        let extension = Path::new(&self.filename)
            .extension()
            .and_then(OsStr::to_str)
            .unwrap();
        match extension {
            ".desktop" => (),
            ".directory" => (),
            ".kdelnk" => {
                err += "File extension .kdelnk is deprecated";
            }
            _ => {
                err += "Unknown File extension";
            }
        };

        let etype = &self.get_default_group().unwrap().type_string.unwrap();
        if extension == ".directory" && !(etype == "Directory") {
            err += &format!("File extension is .directory, but Type is {}", etype);
        } else if extension == ".desktop" && etype == "Directory" {
            err += "Files with Type=Directory should have the extension .directory";
        }

        Ok(())
    }

    /// Get the group with header "Desktop Entry".
    pub fn get_default_group(&self) -> Option<DesktopEntry> {
        // TODO Improve this function
        Some(self.groups[0].clone())
    }

    /// Validates the contents of a desktop entry. The error enum contains warnings.
    pub fn validate(&self) -> Result<()> {
        for group in &self.groups {
            group.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::desktop_entry::DesktopFile;
    #[test]
    fn parse_desktop_file() {
        let filename = "test_files/desktop_entries/test-multiple.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let groups = desktop_file.groups;
        assert_eq!(desktop_file.filename, filename);
        assert_eq!(groups.len(), 2);
    }
    #[test]
    fn parse_groups() {
        use crate::desktop_entry::DEFAULT_GROUP;
        let filename = "test_files/desktop_entries/test-multiple.desktop";
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
        let filename = "test_files/desktop_entries/test-multiple.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let default_group = &desktop_file.groups[0];
        let result = default_group.check_try_exec();
        let sec_group = &desktop_file.groups[1];
        let result2 = sec_group.check_try_exec().is_err();
        assert_eq!(result.is_ok(), true);
        assert_eq!(result2, false);
    }

    #[test]
    fn check_group() {
        let filename = "test_files/desktop_entries/test-multiple.desktop";
        let desktop_file = DesktopFile::from_file(filename).unwrap();
        let groups = desktop_file.groups;
        let default_group = groups.get(0).unwrap();
        assert_eq!(default_group.check_group().is_ok(), true);
        let filename = "test_files/desktop_entries/fail.desktop";
        let desktop_file = DesktopFile::from_file(filename);
        assert_eq!(desktop_file.is_err(), true);
    }
}

fn parse_strings(s: &str) -> Strings {
    s.split(";")
        .map(|x| x.to_string())
        .filter(|x| x.len() > 0)
        .collect::<Strings>()
}

fn parse_locale_strings(key: &str, value: &str) -> Result<Locales> {
    let values = parse_strings(value);
    if key.contains("[") {
        if key.contains("]") {
            let locale_as_vec: Vec<&str> = key.split("[").collect();
            let locale_string = locale_as_vec[1].to_string();
            let lang = LocaleLang::Lang(locale_string);
            let locale_string = Locales { values, lang };
            Ok(locale_string)
        } else {
            Err(Error::from(format!("Malformed locale string {}", key)))
        }
    } else if key.contains("]") {
        Err(Error::from(format!("Malformed locale string {}", key)))
    } else {
        Ok(Locales {
            values,
            lang: LocaleLang::Default,
        })
    }
}

fn locale_strings_from_hashmap(
    key: &str,
    hashmap: &HashMap<String, String>,
) -> Option<LocaleStrings> {
    let keys: Vec<String> = hashmap
        .keys()
        .filter(|x| x.starts_with(key))
        .map(|x| x.clone())
        .collect();
    let mut values: LocaleStrings = vec![];
    if let Some(value) = hashmap.get(key) {
        for key in keys {
            let locale_string = parse_locale_strings(&key, value).unwrap();
            values.push(locale_string)
        }
    } else {
        return None;
    }
    Some(values)
}

fn locale_string_from_hashmap(
    key: &str,
    hashmap: &HashMap<String, String>,
) -> Option<LocaleString> {
    use std::convert::TryInto;

    if let Some(locale_strings) = locale_strings_from_hashmap(key, hashmap) {
        let locale_string: LocaleString = locale_strings
            .iter()
            .map(|x| x.clone().try_into().unwrap())
            .collect();
        Some(locale_string)
    } else {
        None
    }
}

fn get_default_value(locale_string: LocaleString) -> Result<String> {
    let default: Vec<Locale> = locale_string
        .iter()
        .filter(|x| x.lang.is_default())
        .map(|x| x.clone())
        .collect();
    if default.is_empty() {
        Err(Error::from("TODO"))
    } else {
        Ok(default[0].value.clone())
    }
}

impl LocaleLang {
    fn is_default(&self) -> bool {
        match &self {
            Self::Default => true,
            _ => false,
        }
    }
}
