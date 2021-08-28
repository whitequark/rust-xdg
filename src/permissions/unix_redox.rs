use super::Permission;
use std::{fmt, fs, os::unix::fs::PermissionsExt, path::Path};

pub(super) use std::io::Error as GetPathPermsError;
pub(super) use std::io::Error as SetPathPermsError;

#[derive(Copy, Clone)]
pub(crate) struct Permissions(u32);

impl Permissions {
    const GROUP_EVERYONE_MASK: u32 = 0o077;
    #[cfg(test)]
    const OWNER_MASK: u32 = 0o700;
}

impl Permission for Permissions {
    fn from_path(path: &Path) -> Result<Self, super::GetPathPermsError> {
        Ok(Self(
            fs::metadata(path)
                .map_err(super::GetPathPermsError)?
                .permissions()
                .mode(),
        ))
    }

    fn apply_path(&self, path: &Path) -> Result<(), super::SetPathPermsError> {
        let &Permissions(perms) = self;
        fs::set_permissions(path, fs::Permissions::from_mode(perms))
            .map_err(super::SetPathPermsError)
    }

    fn is_only_owner_full_control(&self) -> bool {
        let &Permissions(perms) = self;
        perms & Self::GROUP_EVERYONE_MASK == 0
    }

    #[cfg(test)]
    fn only_owner_full_control() -> Self {
        Self(Self::OWNER_MASK)
    }

    #[cfg(test)]
    fn set_only_owner_full_control(&mut self) {
        let Permissions(perms) = self;
        *perms &= Self::OWNER_MASK;
    }
}

impl fmt::Debug for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Permissions(p) = *self;
        write!(f, "{:#05o}", p)
    }
}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
