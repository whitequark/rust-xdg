use super::*;

// This module implements menu-spec and desktop-entry

pub trait DesktopEntries {
    /// Returns a vector of all menu items according to
    /// https://standards.freedesktop.org/menu-spec/1.1/
    fn list_menu_items(&self) -> Vec<PathBuf>;
}

impl DesktopEntries for BaseDirectories {
    fn list_menu_items(&self) -> Vec<PathBuf> {
        self.list_data_files("applications")
    }
}
