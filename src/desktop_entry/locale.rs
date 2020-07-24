use crate::desktop_entry::parse_strings;
use crate::desktop_entry::Error;
use crate::desktop_entry::{Result, Strings};
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Clone, Debug)]
pub enum LocaleLang {
    Default,
    Lang(String),
}

/// # Example
/// ```
/// use xdg::desktop_entry::DesktopFile;
///
/// let desktop_entry = "
///     [Desktop Entry]
///     Type=Application
///     Name=Foo
///     Name[jp]=銹
///     Name[sp]=ElFoo
///     Exec=Bar
/// ";
///
/// let desktop_entry_file = DesktopFile::from_str(desktop_entry).unwrap();
/// let name = desktop_entry_file.get_name().unwrap();
/// assert_eq!(name, "Foo".to_string());
/// let default_group = desktop_entry_file.get_default_group().unwrap();
/// let v = default_group.name.unwrap();
/// println!("{:?}", v);
/// assert_eq!(v.len(), 3);
/// assert_eq!(v[0].value, "Foo".to_string());
/// assert_eq!(v[1].value, "銹".to_string());
/// assert_eq!(v[2].value, "ElFoo".to_string());
/// ```
///
#[derive(Clone, Debug)]
pub struct Locale {
    pub lang: LocaleLang,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Locales {
    pub lang: LocaleLang,
    pub values: Strings,
}

pub type LocaleString = Vec<Locale>;
pub type LocaleStrings = Vec<Locales>;

impl TryFrom<Locales> for Locale {
    type Error = Error;
    fn try_from(locales: Locales) -> Result<Self> {
        if locales.values.is_empty() {
            Err(Error::from("Could not convert Locales to Locale"))
        } else {
            Ok(
                Self {
                    value: locales.values[0].clone(),
                    lang: locales.lang,
                }
            )
        }
    }
}

impl LocaleLang {
    pub fn is_default(&self) -> bool {
        match &self {
            Self::Default => true,
            _ => false,
        }
    }
}

pub fn get_default_value(locale_string: LocaleString) -> Result<String> {
    let default: Vec<Locale> = locale_string
        .iter()
        .filter(|x| x.lang.is_default())
        .map(|x| x.clone())
        .collect();
    if default.is_empty() {
        Err(Error::from("Default locale is missing"))
    } else {
        Ok(default[0].value.clone())
    }
}

/// # Example
/// ```
/// use xdg::desktop_entry::locale::{parse_locale_strings, LocaleLang};
///
/// let locales = parse_locale_strings("Name[jp]", "銹").unwrap();
/// assert_eq!(&locales.values[0], "銹");
/// if let LocaleLang::Lang(lang) = locales.lang {
///     assert_eq!(&lang, "jp");
/// }
/// ```
pub fn parse_locale_strings(key: &str, value: &str) -> Result<Locales> {
    let values = parse_strings(value);
    if key.contains("[") {
        if key.contains("]") {
            let locale_as_vec: Vec<&str> = key.split("[").collect();
            let locale_string = locale_as_vec[1].trim_end_matches(']').to_string();
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

pub fn locale_strings_from_hashmap(
    key: &str,
    hashmap: &HashMap<String, String>,
) -> Option<LocaleStrings> {
    let keys: Vec<String> = hashmap
        .keys()
        .filter(|x| x.starts_with(key))
        .map(|x| x.clone())
        .collect();
    let mut values: LocaleStrings = vec!();
    for k in keys {
        if let Some(value) = hashmap.get(&k) {
            let locale_string = parse_locale_strings(&k, value).unwrap();
            println!("{:?}", locale_string);
            values.push(locale_string)
        }
    }
    Some(values)
}

pub fn locale_string_from_hashmap(
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
