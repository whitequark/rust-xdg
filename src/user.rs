use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use crate::base::BaseDirectories;
use crate::error::XdgError;
use crate::error::XdgErrorKind::*;
use crate::util::*;

/// UserDirectories allows to lookup paths to common directories like Documents or Music, localized to the user's language, according to [xdg-user-dirs].
/// [xdg-user-dirs]: https://www.freedesktop.org/wiki/Software/xdg-user-dirs/
#[derive(Debug)]
pub struct UserDirectories {
    desktop: Option<PathBuf>,
    download: Option<PathBuf>,
    templates: Option<PathBuf>,
    public_share: Option<PathBuf>,
    documents: Option<PathBuf>,
    music: Option<PathBuf>,
    pictures: Option<PathBuf>,
    videos: Option<PathBuf>,
}

impl UserDirectories {
    pub fn new() -> Result<UserDirectories, XdgError> {
        Self::with_basedir(BaseDirectories::new()?)
    }

    /// Get UserDirectories based on supplied BaseDirectories, required to read $XDG_CONFIG_HOME/user-dirs.dirs
    pub fn with_basedir(basedir: BaseDirectories) -> Result<UserDirectories, XdgError> {
        let home = dirs::home_dir().ok_or_else(|| XdgError::new(HomeMissing))?;

        let user_dirs = basedir.get_config_home().join("user-dirs.dirs");

        if user_dirs.exists() {
            let f = File::open(user_dirs).map_err(|err| XdgError::new(XdgUserDirsOpen(err)))?;
            let mut reader = BufReader::new(f);

            let mut str = String::new();
            reader
                .read_to_string(&mut str)
                .map_err(|err| XdgError::new(XdgUserDirsRead(err)))?;

            let env = dotenv_parser::parse_dotenv(&str)
                .map_err(|_| XdgError::new(XdgUserDirsMalformed))?;

            Ok(UserDirectories {
                desktop: get_userpath(&env, "XDG_DESKTOP_DIR", &home),
                download: get_userpath(&env, "XDG_DOWNLOAD_DIR", &home),
                templates: get_userpath(&env, "XDG_TEMPLATES_DIR", &home),
                public_share: get_userpath(&env, "XDG_PUBLICSHARE_DIR", &home),
                documents: get_userpath(&env, "XDG_DOCUMENTS_DIR", &home),
                music: get_userpath(&env, "XDG_MUSIC_DIR", &home),
                pictures: get_userpath(&env, "XDG_PICTURES_DIR", &home),
                videos: get_userpath(&env, "XDG_VIDEOS_DIR", &home),
            })
        } else {
            Err(XdgError::new(XdgUserDirsMissing))
        }
    }

    pub fn get_desktop(&self) -> Option<&PathBuf> {
        self.desktop.as_ref()
    }

    pub fn get_download(&self) -> Option<&PathBuf> {
        self.download.as_ref()
    }

    pub fn get_templates(&self) -> Option<&PathBuf> {
        self.templates.as_ref()
    }

    pub fn get_public_share(&self) -> Option<&PathBuf> {
        self.public_share.as_ref()
    }

    pub fn get_documents(&self) -> Option<&PathBuf> {
        self.documents.as_ref()
    }

    pub fn get_music(&self) -> Option<&PathBuf> {
        self.music.as_ref()
    }

    pub fn get_pictures(&self) -> Option<&PathBuf> {
        self.pictures.as_ref()
    }

    pub fn get_videos(&self) -> Option<&PathBuf> {
        self.videos.as_ref()
    }
}
