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
