use super::*;

#[test]
fn name_is_the_file_stem_of_the_path() {
    assert_eq!(name_from_argv0(OsStr::new("/usr/bin/meshtown-cli")), "meshtown-cli");
    assert_eq!(name_from_argv0(OsStr::new("meshtown")), "meshtown");
}

#[test]
fn exe_extension_is_dropped() {
    assert_eq!(name_from_argv0(OsStr::new("meshtown-cli.exe")), "meshtown-cli");
}

#[test]
fn empty_name_falls_back_to_meshtown() {
    assert_eq!(name_from_argv0(OsStr::new("")), "meshtown");
}

#[test]
fn name_is_meshtown_until_set() {
    assert_eq!(program_name(), "meshtown");
}
