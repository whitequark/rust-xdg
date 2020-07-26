//! # desktop_entry
//! Implementation of the [XDG Desktop Entry Specification][xdg-desktop-entry].
//!
//! [xdg-desktop-entry]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
//!

// TODO Add custom X- groups support
pub use self::error::Error;
pub use self::locale::*;
use ini::Ini;
use regex::Regex;
use std::collections::HashMap;
use std::fmt;

use std::ffi::OsStr;
use std::path::Path;

mod error;
pub mod locale;

#[cfg(test)]
mod test;

type IconString = String;
type Strings = Vec<String>;

const DEFAULT_GROUP: &str = "Desktop Entry";

/// This type allows to load and validate desktop files according
/// to the [X Desktop Group Desktop File Entry specification][xdg-desktop-file].
///
/// # Examples
///
/// To load a desktop file `foo.desktop`:
///
/// ```
/// use xdg::desktop_entry::DesktopEntry;
///
/// let desktop_file = DesktopEntry::from_file("test_files/desktop_entries/test.desktop").unwrap();
/// let name = desktop_file.get_name().ok();
///
/// assert_eq!(name, Some("Foo".to_string()));
/// ```
///
/// To validate the desktop file:
///
/// ```
/// use xdg::desktop_entry::DesktopEntry;
/// use std::str::FromStr;
///
/// let desktop_entry = "
///     [Desktop Entry]
///     Type=Application
///     Name=Foo
///     Exec=Bar
/// ";
///
/// let desktop_entry_file = DesktopEntry::from_str(desktop_entry).unwrap();
/// let result = desktop_entry_file.validate();
/// assert_eq!(result.is_ok(), true);
/// ```
///
/// To get the default group use the `get_default_group` method.
///
/// [xdg-desktop-file]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
///
pub struct DesktopEntry {
    pub filename: String,
    pub groups: Vec<Group>,
}

/// Individual group header for a desktop file.
/// The struc fields correspond to the possible
/// [recognized desktop entry keys][xdg-keys], with the
/// exception of Type which is replaced with `type_string`.
///
/// [xdg-keys]: https://specifications.freedesktop.org/desktop-entry-spec/latest/ar01s06.html
///
#[derive(Clone)]
pub struct Group {
    pub group_name: String,
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

type Result<T> = std::result::Result<T, Error>;

impl Group {
    pub fn get_name(&self) -> Result<String> {
        let name = &self
            .name
            .clone()
            .ok_or_else(|| Error::from("Could not get name"))?;
        name.get_default()
    }

    pub fn get_header(&self) -> Result<String> {
        Ok(self.group_name.clone())
    }

    pub fn get_type(&self) -> Result<String> {
        let err = Error(vec!["Could not read Type".to_string()]);
        self.type_string.clone().ok_or(err)
    }

    pub fn get_exec(&self) -> Result<String> {
        let err = Error::from("Could not read Exec");
        self.exec.clone().ok_or(err)
    }

    pub fn get_url(&self) -> Result<String> {
        let err = Error::from("Could not read URL");
        self.url.clone().ok_or(err)
    }

    fn from_hash_map(group_name: String, hashmap: &HashMap<String, String>) -> Result<Self> {
        // String type
        let type_string = hashmap.get("Type").cloned();
        let version = hashmap.get("Version").cloned();
        let exec = hashmap.get("Exec").cloned();
        let try_exec = hashmap.get("TryExec").cloned();
        let path = hashmap.get("Path").cloned();
        let startup_wm_class = hashmap.get("StartupWMClass").cloned();
        let url = hashmap.get("URL").cloned();
        // IconString
        let icon = hashmap.get("Icon").cloned();
        // LocalString type
        let name = LocaleString::from_hashmap("Name", hashmap)?;
        let generic_name = LocaleString::from_hashmap("GenericName", hashmap)?;
        let comment = LocaleString::from_hashmap("Comment", hashmap)?;
        // LocaleStrings type
        let keywords = LocaleStrings::from_hashmap("Keywords", hashmap)?;
        // Bool type
        let no_display = hashmap.get("NoDisplay").parse()?;
        let hidden = hashmap.get("Hidden").parse()?;
        let dbus_activatable = hashmap.get("DBusActivatable").parse()?;
        let terminal = hashmap.get("Terminal").parse()?;
        let startup_notify = hashmap.get("StartupNotify").parse()?;
        let prefers_non_default_gpu = hashmap.get("PrefersNonDefaultGPU").parse()?;
        // Strings type
        let only_show_in = hashmap.get("OnlyShowIn").parse()?;
        let not_show_in = hashmap.get("NotShowIn").parse()?;
        let actions = hashmap.get("Actions").parse()?;
        let mime_type = hashmap.get("MimeType").parse()?;
        let categories = hashmap.get("Categories").parse()?;
        let implements = hashmap.get("Implements").parse()?;

        let desktop_entry = Group {
            group_name,
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
            if !warning.is_empty() {
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
            if !warning.is_empty() {
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
        let re = Regex::new(r"^Desktop Action [a-zA-Z0-9-]+$")
            .map_err(|_| Error::from("Could not parse regex"))?;
        let group: &str = &self.group_name;
        let mut err: Vec<String> = vec![];
        if !(group == DEFAULT_GROUP
            || re.is_match(group)
            || group.starts_with("X-") && group.is_ascii())
        {
            err.push(format!("Invalid Group name: {}", group));
        } else if self.only_show_in.is_some() && self.not_show_in.is_some() {
            err.push("Group may either have OnlyShowIn or NotShowIn, but not both".to_string());
        }
        if !err.is_empty() {
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
            if !x.is_empty() {
                return Err(Error(x));
            }
        }
        Ok(())
    }

    fn is_default_grop(&self) -> bool {
        // TODO verify there are no more cases
        let group: &str = &self.group_name;
        group == DEFAULT_GROUP
    }

    fn check_extras(&self) -> Result<()> {
        let group = &self.group_name;
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

        if !err.is_empty() {
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

            if etype == "Application" && self.exec.is_none() {
                warnings.push("Type=Application needs 'Exec' key".to_string());
            }

            if etype == "Link" && self.url.is_none() {
                warnings.push("Type=Link needs 'URL' key".to_string());
            }
        }

        if self.only_show_in.is_some() {
            self.check_only_show_in()?;
        }

        if self.not_show_in.is_some() {
            self.check_not_show_in()?;
        }

        if !warnings.is_empty() {
            Err(Error(warnings))
        } else {
            Ok(())
        }
    }

    /// Validates the group, the error `Err(error)` contains the warnings.
    pub fn validate(&self) -> Result<()> {
        self.check_keys()?;
        self.check_group()?;
        self.check_extras()?;
        self.check_try_exec()?;
        self.check_not_show_in()?;
        self.check_only_show_in()?;
        self.check_categories()?;
        Ok(())
    }
}

/// Writes the contents of a `DesktopEntry` to a file `filename`.
///
/// ```
/// use xdg::desktop_entry::DesktopEntry;
/// use std::fs::File;
/// use std::io::prelude::*;
/// use std::error::Error;
/// use std::str::FromStr;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let desktop_entry_contents = "[Desktop Entry]\n\
///     Type=Application\n\
///     Exec=Bar\n\
///     Name=Foo\n\
///     Terminal=true";
///     let d_entry = DesktopEntry::from_str(desktop_entry_contents)?;
///     d_entry.to_file("foo.desktop")?;
///
///     let mut file = File::open("foo.desktop")?;
///     let mut s = String::new();
///     let contents = file.read_to_string(&mut s)?;
///     // Note that the order of the lines in the generated file is deterministic,
///     // and could not coincide with the original file.
///     assert_eq!(s, desktop_entry_contents);
///     Ok(())
/// }
/// ```
impl DesktopEntry {
    pub fn to_file(&self, filename: impl AsRef<Path>) -> Result<()> {
        use std::fs::File;
        use std::io::prelude::*;

        let err = Error::from(format!(
            "Could not create file {}",
            filename.as_ref().display()
        ));
        let mut file = File::create(filename).map_err(|_| err)?;
        if file.write_all(self.to_string().as_bytes()).is_ok() {
            Ok(())
        } else {
            Err(Error::from("Could not write"))
        }
    }

    pub fn get_name(&self) -> Result<String> {
        self.get_default_group()?.get_name()
    }

    pub fn get_type(&self) -> Result<String> {
        self.get_default_group()?.get_type()
    }

    pub fn get_exec(&self) -> Result<String> {
        self.get_default_group()?.get_exec()
    }

    pub fn get_url(&self) -> Result<String> {
        self.get_default_group()?.get_url()
    }

    fn load_ini(ini: impl AsRef<Path>) -> Result<Vec<(String, HashMap<String, String>)>> {
        let err = Error::from(format!("Could not load ini {}", ini.as_ref().display()));
        let i = Ini::load_from_file(ini).map_err(|_| err)?;
        let mut result = vec![];
        for (sec, prop) in i.iter() {
            let mut s = HashMap::new();
            for (k, v) in prop.iter() {
                s.insert(k.to_string(), v.to_string());
            }
            result.push((
                sec.ok_or_else(|| Error::from(format!("Could not read {:?}", sec)))?
                    .to_string(),
                s,
            ));
        }
        Ok(result)
    }

    fn from_hash_map(
        hash: &[(String, HashMap<String, String>)],
        filename: impl AsRef<Path>,
    ) -> Result<Self> {
        let mut groups = vec![];
        for (entry_name, entry) in hash.iter() {
            groups.push(Group::from_hash_map(entry_name.into(), &entry)?);
        }
        let name = filename
            .as_ref()
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                Error::from(format!(
                    "Could not convert file_name of {} to String",
                    filename.as_ref().display()
                ))
            })?
            .to_string();
        let desktop_file = Self {
            filename: name,
            groups,
        };
        desktop_file.check_extension()?;
        desktop_file.validate()?;
        Ok(desktop_file)
    }

    /// Load a `DesktopEntry` from a file `filename`.
    pub fn from_file(filename: impl AsRef<Path>) -> Result<Self> {
        let hash = Self::load_ini(&filename)?;
        Self::from_hash_map(
            &hash,
            filename.as_ref().to_str().ok_or(Error::from(format!(
                "Could not convert {}",
                filename.as_ref().display()
            )))?,
        )
    }

    fn check_extension(&self) -> Result<()> {
        let mut err = String::new();
        let extension = Path::new(&self.filename)
            .extension()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                Error::from(format!(
                    "Could not convert extension of {} to String",
                    &self.filename
                ))
            })?;
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

        if let Some(etype) = &self.get_default_group()?.type_string {
            if extension == ".directory" && etype != "Directory" {
                err += &format!("File extension is .directory, but Type is {}", etype);
            } else if extension == ".desktop" && etype == "Directory" {
                err += "Files with Type=Directory should have the extension .directory";
            }
        } else {
            return Err(Error::from("key 'Type' is missing"));
        }

        Ok(())
    }

    /// Get the group with header "Desktop Entry".
    ///
    /// # Example
    ///
    /// ```
    /// use xdg::desktop_entry::DesktopEntry;
    /// use std::str::FromStr;
    ///
    /// let desktop_entry = "
    ///     [Desktop Entry]
    ///     Type=Application
    ///     Name=Foo
    ///     Exec=Bar
    ///
    ///     [Desktop Action Bar]
    ///     Exec=foobar
    ///     Name=Foo Bar
    /// ";
    ///
    /// let desktop_entry_file = DesktopEntry::from_str(desktop_entry).unwrap();
    /// let name = desktop_entry_file.get_name().unwrap();
    /// assert_eq!(name, "Foo");
    /// let second_group = desktop_entry_file.groups[1].clone();
    /// assert_eq!(second_group.get_name().unwrap(), "Foo Bar");
    /// assert_eq!(second_group.group_name, "Desktop Action Bar");
    /// ```
    ///
    pub fn get_default_group(&self) -> Result<Group> {
        // TODO Improve this function
        Ok(self.groups[0].clone())
    }

    /// Validates the contents of a desktop entry. The error enum contains warnings.
    pub fn validate(&self) -> Result<()> {
        for group in &self.groups {
            group.validate()?;
        }
        Ok(())
    }
}

impl fmt::Display for DesktopEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut string = String::new();
        for group in &self.groups {
            string += "\n";
            string += &group.to_string();
        }
        let string = string.trim_start_matches('\n').to_string();
        write!(f, "{}", string)
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut string = format!("[{}]", self.group_name);
        let mut append_string = |opt: &Option<String>, key: &str| {
            if let Some(s) = opt {
                string += &format!("\n{}={}", key, &s);
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
                for locale in locale_string.locs.iter() {
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
                string += &format!("\n{}={}", key, &s);
            };
        };
        append_bool(&self.terminal, "Terminal");
        append_bool(&self.no_display, "NoDisplay");
        append_bool(&self.hidden, "Hidden");
        append_bool(&self.dbus_activatable, "DBusActivatable");
        append_bool(&self.startup_notify, "StartupNotify");
        append_bool(&self.prefers_non_default_gpu, "PrefersNonDefaultGPU");
        append_bool(&self.no_display, "NoDisplay");

        let mut append_strings = |opt: &Option<Strings>, key: &str| {
            if let Some(s) = opt {
                let values = s.join(";");
                string += &format!("\n{}={};", key, values)
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
                for locale in locale_strings.locs.iter() {
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

pub trait Parse<T> {
    fn parse(&self) -> Result<Option<T>>;
}

impl Parse<bool> for Option<&String> {
    fn parse(&self) -> Result<Option<bool>> {
        if let Some(s) = self {
            use std::str::FromStr;

            let err = Error::from(format!("{} is not a valid boolean", s));
            FromStr::from_str(s).map_err(|_| err).map(Some)
        } else {
            Ok(None)
        }
    }
}

impl Parse<Strings> for Option<&String> {
    fn parse(&self) -> Result<Option<Strings>> {
        if let Some(s) = self {
            let string = s
                .split(';')
                .map(|x| x.to_string())
                .filter(|x| !x.is_empty())
                .collect::<Strings>();
            if string.is_empty() {
                Err(Error::from(format!(
                    "{} is not a valid sequence of strings",
                    s
                )))
            } else {
                Ok(Some(string))
            }
        } else {
            Ok(None)
        }
    }
}

/// Loads a desktop entry from a string.
///
/// # Example
///
/// ```
/// use xdg::desktop_entry::DesktopEntry;
/// use std::str::FromStr;
///
/// let desktop_entry = "
///     [Desktop Entry]
///     Type=Application
///     Name=Foo
///     Exec=Bar
/// ";
/// let loaded_entry = DesktopEntry::from_str(desktop_entry).unwrap();
/// assert_eq!(loaded_entry.get_name().unwrap(), "Foo".to_string());
/// ```
impl std::str::FromStr for DesktopEntry {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let i = Ini::load_from_str(s)
            .map_err(|_| Error::from(format!("Could not load ini from {}", s)))?;

        let mut result = vec![];
        for (sec, prop) in i.iter() {
            let mut s = HashMap::new();
            for (k, v) in prop.iter() {
                s.insert(k.to_string(), v.to_string());
            }
            result.push((
                sec.ok_or_else(|| Error::from(format!("Could not read {:?}", sec)))?
                    .to_string(),
                s,
            ));
        }
        let desktop_file = Self::from_hash_map(&result, "str.desktop")?;
        Ok(desktop_file)
    }
}
