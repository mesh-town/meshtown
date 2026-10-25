use super::*;

#[test]
fn name_is_the_file_stem_of_the_path() {
    assert_eq!(
        name_from_argv0(OsStr::new("/usr/bin/meshtown-cli")),
        "meshtown-cli"
    );
    assert_eq!(name_from_argv0(OsStr::new("meshtown")), "meshtown");
}

#[test]
fn exe_extension_is_dropped() {
    assert_eq!(
        name_from_argv0(OsStr::new("meshtown-cli.exe")),
        "meshtown-cli"
    );
}

#[test]
fn version_suffix_is_not_an_extension() {
    assert_eq!(
        name_from_argv0(OsStr::new("meshtown-macos-cli-0.3.50")),
        "meshtown-macos-cli-0.3.50"
    );
}

#[test]
fn exe_extension_is_dropped_in_any_case() {
    assert_eq!(name_from_argv0(OsStr::new("MESHTOWN.EXE")), "MESHTOWN");
    assert_eq!(name_from_argv0(OsStr::new("a/meshtown.Exe")), "meshtown");
}

#[test]
fn empty_name_falls_back_to_meshtown() {
    assert_eq!(name_from_argv0(OsStr::new("")), "meshtown");
}

#[test]
fn name_is_meshtown_until_set() {
    assert_eq!(program_name(), "meshtown");
}
