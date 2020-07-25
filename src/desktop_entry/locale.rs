use crate::desktop_entry::Error;
use crate::desktop_entry::{Parse, Result, Strings};
use std::collections::HashMap;
use std::convert::TryFrom;

#[derive(Clone, Debug)]
pub enum LocaleLang {
    Default,
    Lang(String),
}

/// A `LocaleString` is a collection of pairs `lang`, `value`
/// which are represented by the `Locale` struct.
///
/// # Example
/// ```
/// use xdg::desktop_entry::DesktopEntry;
/// use std::str::FromStr;
///
/// let desktop_entry = "
///     [Desktop Entry]
///     Type=Application
///     Name=Foo
///     Name[jp]=銹
///     Name[es]=ElFoo
///     Exec=Bar
///     Keywords=a;b;
///     Keywords[es]=c;d;
/// ";
///
/// let desktop_entry_file = DesktopEntry::from_str(desktop_entry).unwrap();
/// let name = desktop_entry_file.get_name().unwrap();
/// assert_eq!(name, "Foo".to_string());
/// let default_group = desktop_entry_file.get_default_group().unwrap();
/// let name = default_group.name.unwrap();
/// assert_eq!(name.len(), 3);
/// assert_eq!(name.get_default().unwrap(), "Foo".to_string());
/// assert_eq!(name.get("es").unwrap(), "ElFoo".to_string());
/// assert_eq!(name.get("jp").unwrap(), "銹".to_string());
/// let keywords = default_group.keywords.unwrap();
/// assert_eq!(keywords.get_default().unwrap(), ["a", "b"]);
/// assert_eq!(keywords.get("es").unwrap(), ["c", "d"]);
/// assert_eq!(keywords.len(), 2);
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

#[derive(Clone, Debug)]
pub struct LocaleString {
    pub locs: Vec<Locale>,
}

#[derive(Clone, Debug)]
pub struct LocaleStrings {
    pub locs: Vec<Locales>,
}

impl TryFrom<Locales> for Locale {
    type Error = Error;
    fn try_from(locales: Locales) -> Result<Self> {
        if locales.values.is_empty() {
            Err(Error::from("Could not convert Locales to Locale"))
        } else {
            Ok(Self {
                value: locales.values[0].clone(),
                lang: locales.lang,
            })
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

impl LocaleString {
    pub fn len(&self) -> usize {
        self.locs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.locs.len() == 0
    }

    pub fn get(&self, lang: &str) -> Result<String> {
        let lang = lang.to_string();
        for locale in self.locs.iter() {
            if let LocaleLang::Lang(locale_lang) = locale.lang.clone() {
                if lang == locale_lang {
                    let val = locale.value.clone();
                    return Ok(val);
                }
            }
        }
        Err(Error::from(""))
    }

    pub fn get_default(&self) -> Result<String> {
        let default: Vec<Locale> = self
            .locs
            .iter()
            .filter(|x| x.lang.is_default())
            // .map(|x| x.clone())
            .cloned()
            .collect();
        if default.is_empty() {
            Err(Error::from("Default locale is missing"))
        } else {
            Ok(default[0].value.clone())
        }
    }

    pub fn from_hashmap(key: &str, hashmap: &HashMap<String, String>) -> Option<LocaleString> {
        use std::convert::TryInto;

        if let Some(locale_strings) = LocaleStrings::from_hashmap(key, hashmap) {
            let locale_string: Vec<Locale> = locale_strings
                .locs
                .iter()
                .map(|x| x.clone().try_into().unwrap())
                .collect();
            Some(LocaleString {
                locs: locale_string,
            })
        } else {
            None
        }
    }
}

impl LocaleStrings {
    pub fn len(&self) -> usize {
        self.locs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.locs.is_empty()
    }

    pub fn get_default(&self) -> Result<Strings> {
        let default: Vec<Locales> = self
            .locs
            .iter()
            .filter(|x| x.lang.is_default())
            .cloned()
            // .map(|x| x.clone())
            .collect();
        if default.is_empty() {
            Err(Error::from("Default locale is missing"))
        } else {
            Ok(default[0].values.clone())
        }
    }

    pub fn get(&self, lang: &str) -> Result<Strings> {
        let lang = lang.to_string();
        for locale in self.locs.iter() {
            if let LocaleLang::Lang(locale_lang) = locale.lang.clone() {
                if lang == locale_lang {
                    let val = locale.values.clone();
                    return Ok(val);
                }
            }
        }
        Err(Error::from(""))
    }

    pub fn from_hashmap(key: &str, hashmap: &HashMap<String, String>) -> Option<LocaleStrings> {
        let keys: Vec<String> = hashmap
            .keys()
            .filter(|x| x.starts_with(key))
            .cloned()
            .collect();
        let mut values = vec![];
        for k in keys {
            if let Some(value) = hashmap.get(&k) {
                let locale_string = parse_locale_strings(&k, value).ok()?;
                values.push(locale_string)
            }
        }
        Some(LocaleStrings { locs: values })
    }
}

/// Turns `Key[lang], Val` into a `Locales {lang, value: Val}`.
///
/// # Example
/// ```
/// use xdg::desktop_entry::{parse_locale_strings, LocaleLang};
///
/// let locales = parse_locale_strings("Name[jp]", "銹").unwrap();
/// assert_eq!(&locales.values[0], "銹");
/// if let LocaleLang::Lang(lang) = locales.lang {
///     assert_eq!(&lang, "jp");
/// }
/// ```
pub fn parse_locale_strings(key: &str, value: &str) -> Result<Locales> {
    let ptr = &value.to_string();
    let v = Some(ptr);
    let values = v
        .parse()?
        .ok_or_else(|| Error::from(format!("Could not read {}", value)))?;
    if key.contains('[') {
        if key.ends_with(']') {
            let locale_as_vec: Vec<&str> = key.split('[').collect();
            let locale_string = locale_as_vec[1].trim_end_matches(']').to_string();
            let lang = LocaleLang::Lang(locale_string);
            let locale_string = Locales { values, lang };
            Ok(locale_string)
        } else {
            Err(Error::from(format!("Malformed locale string {}", key)))
        }
    } else if key.ends_with(']') {
        Err(Error::from(format!("Malformed locale string {}", key)))
    } else {
        Ok(Locales {
            values,
            lang: LocaleLang::Default,
        })
    }
}
