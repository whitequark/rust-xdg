//! This module is a compatibility shim that implements the smallest subset of permissions
//! necessary to implement this library portable on all supported platforms.

use std::{error::Error, fmt, path::Path};

#[cfg(any(unix, target_os = "redox"))]
mod unix_redox;
#[cfg(any(unix, target_os = "redox"))]
use self::unix_redox as impl_;

pub(crate) trait Permission
where
    Self: fmt::Debug + fmt::Display + Sized,
{
    fn from_path(path: &Path) -> Result<Self, GetPathPermsError>;
    fn apply_path(&self, path: &Path) -> Result<(), SetPathPermsError>;
    fn is_only_owner_full_control(&self) -> bool;

    #[cfg(test)]
    fn only_owner_full_control() -> Self;

    #[cfg(test)]
    fn set_only_owner_full_control(&mut self);
}

pub(crate) struct Permissions(impl_::Permissions);

impl Permission for Permissions {
    fn from_path(path: &Path) -> Result<Self, GetPathPermsError> {
        Ok(Self(impl_::Permissions::from_path(path)?))
    }

    fn apply_path(&self, path: &Path) -> Result<(), SetPathPermsError> {
        let &Permissions(ref inner) = self;
        Permission::apply_path(inner, path)
    }

    fn is_only_owner_full_control(&self) -> bool {
        let &Permissions(ref inner) = self;
        Permission::is_only_owner_full_control(inner)
    }

    #[cfg(test)]
    fn only_owner_full_control() -> Self {
        Self(impl_::Permissions::only_owner_full_control())
    }

    #[cfg(test)]
    fn set_only_owner_full_control(&mut self) {
        let &mut Permissions(ref mut inner) = self;
        Permission::set_only_owner_full_control(inner)
    }
}

impl fmt::Debug for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &Permissions(ref inner) = self;
        fmt::Debug::fmt(inner, f)
    }
}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &Permissions(ref inner) = self;
        fmt::Display::fmt(inner, f)
    }
}

pub struct GetPathPermsError(impl_::GetPathPermsError);

impl fmt::Debug for GetPathPermsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &GetPathPermsError(ref inner) = self;
        fmt::Debug::fmt(inner, f)
    }
}

impl fmt::Display for GetPathPermsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &GetPathPermsError(ref inner) = self;
        fmt::Display::fmt(inner, f)
    }
}

impl Error for GetPathPermsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        let &GetPathPermsError(ref inner) = self;
        Some(inner)
    }
}

pub struct SetPathPermsError(impl_::SetPathPermsError);

impl fmt::Debug for SetPathPermsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &SetPathPermsError(ref inner) = self;
        fmt::Debug::fmt(inner, f)
    }
}

impl fmt::Display for SetPathPermsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &SetPathPermsError(ref inner) = self;
        fmt::Display::fmt(inner, f)
    }
}

impl Error for SetPathPermsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        let &SetPathPermsError(ref inner) = self;
        Some(inner)
    }
}
