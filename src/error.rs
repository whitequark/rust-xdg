use std::convert;
use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::Permissions;

use self::XdgErrorKind::*;

pub struct XdgError {
    kind: XdgErrorKind,
}

impl XdgError {
    pub(crate) fn new(kind: XdgErrorKind) -> XdgError {
        XdgError {
            kind,
        }
    }
}

impl fmt::Debug for XdgError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl error::Error for XdgError {
    fn description(&self) -> &str {
        match self.kind {
            HomeMissing => "$HOME must be set",
            XdgRuntimeDirInaccessible(_, _) =>
                "$XDG_RUNTIME_DIR must be accessible by the current user",
            XdgRuntimeDirInsecure(_, _) =>
                "$XDG_RUNTIME_DIR must be secure: have permissions 0700",
            XdgRuntimeDirMissing =>
                "$XDG_RUNTIME_DIR is not set",
            XdgUserDirsMissing => "$XDG_CONFIG_HOME/user-dirs.dirs must exist",
            XdgUserDirsOpen(_) => "$XDG_CONFIG_HOME/user-dirs.dirs must be accessible",
            XdgUserDirsRead(_) => "$XDG_CONFIG_HOME/user-dirs.dirs must be readable",
            XdgUserDirsMalformed => "$XDG_CONFIG_HOME/user-dirs.dirs must contain valid data",
        }
    }
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.kind {
            XdgRuntimeDirInaccessible(_, ref e)
            | XdgUserDirsOpen(ref e)
            | XdgUserDirsRead(ref e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for XdgError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            HomeMissing => write!(f, "$HOME must be set"),
            XdgRuntimeDirInaccessible(ref dir, ref error) => {
                write!(f, "$XDG_RUNTIME_DIR (`{}`) must be accessible \
                           by the current user (error: {})", dir.display(), error)
            },
            XdgRuntimeDirInsecure(ref dir, permissions) => {
                write!(f, "$XDG_RUNTIME_DIR (`{}`) must be secure: must have \
                           permissions 0o700, got {}", dir.display(), permissions)
            },
            XdgRuntimeDirMissing => {
                write!(f, "$XDG_RUNTIME_DIR must be set")
            },
            XdgUserDirsMissing => write!(f, "$XDG_CONFIG_HOME/user-dirs.dirs must exist"),
            XdgUserDirsOpen(ref error) => {
                write!(f, "$XDG_CONFIG_HOME/user-dirs.dirs open failure: error: {}", error)
            },
            XdgUserDirsRead(ref error) => {
                write!(f, "$XDG_CONFIG_HOME/user-dirs.dirs read failure: error: {}", error)
            },
            XdgUserDirsMalformed => write!(f, "$XDG_CONFIG_HOME/user-dirs.dirs malformed data, must be valid user-dirs.dirs file"),
        }
    }
}

impl convert::From<XdgError> for io::Error {
    fn from(error: XdgError) -> io::Error {
        match error.kind {
            HomeMissing | XdgRuntimeDirMissing =>
                io::Error::new(io::ErrorKind::NotFound, error),
            _ => io::Error::new(io::ErrorKind::Other, error)
        }
    }

}

#[derive(Debug)]
pub(crate) enum XdgErrorKind {
    HomeMissing,
    XdgRuntimeDirInaccessible(PathBuf, io::Error),
    XdgRuntimeDirInsecure(PathBuf, Permissions),
    XdgRuntimeDirMissing,

	XdgUserDirsMissing,
	XdgUserDirsOpen(io::Error),
	XdgUserDirsRead(io::Error),
	XdgUserDirsMalformed,
}