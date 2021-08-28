//! Implements Windows platform support for [`super::Permissions`] via the [NT security
//! descriptor API].
//!
//! [NT security descriptor API]: https://docs.microsoft.com/en-us/windows/win32/secauthz/security-descriptors

use super::Permission;
use std::{error::Error, fmt, io, path::Path};
use windows_permissions::{
    constants::{AccessRights, AceType, SeObjectType, SecurityInformation},
    wrappers::{GetNamedSecurityInfo, SetNamedSecurityInfo},
    LocalBox, SecurityDescriptor, Sid,
};

#[derive(Debug)]
pub(super) struct Permissions {
    security_desc: LocalBox<SecurityDescriptor>,
}

impl Permissions {
    const SEC_OBJ_TYPE: SeObjectType = SeObjectType::SE_FILE_OBJECT;

    fn sec_info() -> SecurityInformation {
        SecurityInformation::Dacl | SecurityInformation::Owner
    }
}

impl fmt::Display for Permissions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(stringify!(Permissions))
            .field(
                "is_only_owner_full_control",
                &self.is_only_owner_full_control(),
            )
            .finish()
    }
}

impl Permission for Permissions {
    fn from_path(path: &Path) -> Result<Self, super::GetPathPermsError> {
        let security_desc = GetNamedSecurityInfo(path, Self::SEC_OBJ_TYPE, Self::sec_info())
            .map_err(GetPathPermsError::FetchSecurityDescriptor)?;

        Ok(Self { security_desc })
    }

    fn apply_path(&self, path: &Path) -> Result<(), super::SetPathPermsError> {
        let &Permissions { ref security_desc } = self;

        Ok(SetNamedSecurityInfo(
            path,
            Self::SEC_OBJ_TYPE,
            Self::sec_info(),
            security_desc.owner(),
            None,
            security_desc.dacl(),
            None,
        )
        .map_err(SetPathPermsError::SetSecurityDescriptor)?)
    }

    fn is_only_owner_full_control(&self) -> bool {
        let &Permissions { ref security_desc } = self;

        let owner = security_desc
            .owner()
            .expect("logic error: expected owner of security descriptor to be present");

        // You can validate the constants used here for at
        // https://docs.microsoft.com/en-us/windows/win32/secauthz/well-known-sids

        // `SECURITY_LOCAL_SYSTEM_RID`
        let system_rid = Sid::new([0, 0, 0, 0, 0, 5], &[18]).unwrap();
        // `DOMAIN_ALIAS_RID_ADMINS`
        let admins_rid = Sid::new([0, 0, 0, 0, 0, 5], &[32, 544]).unwrap();

        // Drawing on knowledge from
        // https://docs.microsoft.com/en-us/windows/win32/secauthz/dacls-and-aces:
        //
        // > If a Windows object does not have a discretionary access control list (DACL), the
        // > system allows everyone full access to it. If an object has a DACL, the system allows
        // > only the access that is explicitly allowed by the access control entries (ACEs) in the
        // > DACL. If there are no ACEs in the DACL, the system does not allow access to anyone.
        match security_desc.dacl() {
            None => false, // No ACL = anyone can access.
            Some(dacl) => match dacl.len() {
                // Empty ACL = nobody can access. Owner can change perms, but doesn't have direct
                // access ATM.
                0 => false,
                num_aces => (0..num_aces).all(|ace_idx| {
                    let ace = dacl.get_ace(ace_idx).unwrap();
                    match ace.ace_type() {
                        AceType::ACCESS_ALLOWED_ACE_TYPE
                        | AceType::ACCESS_ALLOWED_CALLBACK_ACE_TYPE
                        | AceType::ACCESS_ALLOWED_CALLBACK_OBJECT_ACE_TYPE
                        | AceType::ACCESS_ALLOWED_OBJECT_ACE_TYPE => {
                            // `SYSTEM` and `Administrators` are perfectly reasonable groups to
                            // allow.
                            let sid = ace.sid().unwrap();
                            [system_rid.as_ref(), admins_rid.as_ref()]
                                .iter()
                                .any(|whitelisted_sid| whitelisted_sid == &sid)
                                || (sid == owner
                                    && ace.mask().contains(AccessRights::FileAllAccess))
                        }
                        AceType::ACCESS_DENIED_ACE_TYPE
                        | AceType::ACCESS_DENIED_CALLBACK_ACE_TYPE
                        | AceType::ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE
                        | AceType::ACCESS_DENIED_OBJECT_ACE_TYPE => {
                            let sid = ace.sid().unwrap();
                            // TODO: Is this correct? What do we need to refine here?
                            sid != owner || ace.mask().intersects(AccessRights::FileAllAccess)
                        }
                        // These don't affect the access we care about.
                        AceType::SYSTEM_AUDIT_ACE_TYPE
                        | AceType::SYSTEM_AUDIT_CALLBACK_ACE_TYPE
                        | AceType::SYSTEM_AUDIT_CALLBACK_OBJECT_ACE_TYPE
                        | AceType::SYSTEM_AUDIT_OBJECT_ACE_TYPE
                        | AceType::SYSTEM_MANDATORY_LABEL_ACE_TYPE
                        | AceType::SYSTEM_RESOURCE_ATTRIBUTE_ACE_TYPE
                        | AceType::SYSTEM_SCOPED_POLICY_ID_ACE_TYPE => true,
                    }
                }),
            },
        }
    }

    #[cfg(test)]
    fn only_owner_full_control() -> Self {
        // TODO: Build an ACE giving the owner full perms, make it the sole entry in the DACL of a
        // new `SecurityDescriptor`. Set the DACL of the security descriptor as protected to avoid
        // object inheritance overriding the perm.

        todo!()
        // SecurityInformation::Dacl
        // | SecurityInformation::Group
    }

    #[cfg(test)]
    fn set_only_owner_full_control(&mut self) {
        *self = Self::only_owner_full_control();
    }
}

#[test]
fn dacls_of_things() {
    assert!(Permissions::from_path(
        r"C:\Users\K0RYU\AppData\Roaming\alacritty\alacritty.yml".as_ref()
    )
    .unwrap()
    .is_only_owner_full_control());
}

#[derive(Debug)]
pub(super) enum GetPathPermsError {
    FetchSecurityDescriptor(io::Error),
}

impl From<GetPathPermsError> for super::GetPathPermsError {
    fn from(e: GetPathPermsError) -> Self {
        Self(e)
    }
}

impl fmt::Display for GetPathPermsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &GetPathPermsError::FetchSecurityDescriptor(_) => {
                write!(f, "failed to fetch security descriptor")
            }
        }
    }
}

impl Error for GetPathPermsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            &GetPathPermsError::FetchSecurityDescriptor(ref e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub(super) enum SetPathPermsError {
    SetSecurityDescriptor(io::Error),
}

impl From<SetPathPermsError> for super::SetPathPermsError {
    fn from(e: SetPathPermsError) -> Self {
        Self(e)
    }
}

impl fmt::Display for SetPathPermsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &SetPathPermsError::SetSecurityDescriptor(_) => {
                write!(f, "failed to set security descriptor")
            }
        }
    }
}

impl Error for SetPathPermsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            &SetPathPermsError::SetSecurityDescriptor(ref e) => Some(e),
        }
    }
}
