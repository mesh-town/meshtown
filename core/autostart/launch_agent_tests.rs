use std::path::{Path, PathBuf};

use super::*;

#[test]
fn open_app_plist_golden() {
    // given: a launch through LaunchServices
    let launch = MacLaunch::Open(PathBuf::from("/Applications/MeshTown.app"));
    // when
    let text = launch_agent_plist("app.example.meshtown", &launch);
    // then
    let expected = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>app.example.meshtown</string>
	<key>ProgramArguments</key>
	<array>
		<string>/usr/bin/open</string>
		<string>-a</string>
		<string>/Applications/MeshTown.app</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
</dict>
</plist>
"#;
    assert_eq!(text, expected);
}

#[test]
fn exe_plist_golden() {
    // given: a launch of a bare binary
    let launch = MacLaunch::Exe(PathBuf::from("/opt/mesh/meshtown-macos"));
    // when
    let text = launch_agent_plist(FALLBACK_LABEL, &launch);
    // then
    let expected = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>local.meshtown</string>
	<key>ProgramArguments</key>
	<array>
		<string>/opt/mesh/meshtown-macos</string>
		<string>tray</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
</dict>
</plist>
"#;
    assert_eq!(text, expected);
}

#[test]
fn xml_escapes_path() {
    // given: a path with every XML special character
    let launch = MacLaunch::Exe(PathBuf::from(r#"/Apps/A&B <x> "q" 'z'/meshtown"#));
    // when
    let text = launch_agent_plist("a&b", &launch);
    // then
    assert!(
        text.contains(
            "<string>/Apps/A&amp;B &lt;x&gt; &quot;q&quot; &apos;z&apos;/meshtown</string>"
        ),
        "path is escaped: {text}"
    );
    assert!(
        text.contains("<string>a&amp;b</string>"),
        "label is escaped: {text}"
    );
}

#[test]
fn label_from_bundle_or_fallback() {
    // given
    let app = Path::new("/Applications/MeshTown.app");
    let exe = Path::new("/opt/mesh/meshtown-macos");
    // when
    let (bundled_label, bundled) = mac_launch(Some((app, "app.example.meshtown")), exe);
    let (bare_label, bare) = mac_launch(None, exe);
    // then
    assert_eq!(bundled_label, "app.example.meshtown");
    assert!(
        matches!(bundled, MacLaunch::Open(ref p) if p == app),
        "bundle is opened"
    );
    assert_eq!(bare_label, FALLBACK_LABEL);
    assert!(
        matches!(bare, MacLaunch::Exe(ref p) if p == exe),
        "bare binary is run"
    );
}

#[test]
fn path_is_under_launch_agents() {
    let path = launch_agent_path(Path::new("/Users/example"), "local.meshtown");
    let expected = Path::new("/Users/example")
        .join("Library")
        .join("LaunchAgents")
        .join("local.meshtown.plist");
    assert_eq!(path, expected);
}

#[test]
fn disabled_key_is_switched_off() {
    // given
    let off = "<dict>\n  <key>Disabled</key>\n  <true/>\n</dict>";
    let off_inline = "<key>Disabled</key><true/>";
    let on = "<dict>\n  <key>Disabled</key>\n  <false/>\n</dict>";
    let other_key = "<key>RunAtLoad</key>\n<true/>";
    let own = launch_agent_plist(FALLBACK_LABEL, &MacLaunch::Exe(PathBuf::from("/x")));
    // when / then
    assert!(launch_agent_switched_off(off), "Disabled true");
    assert!(
        launch_agent_switched_off(off_inline),
        "no whitespace between"
    );
    assert!(!launch_agent_switched_off(on), "Disabled false");
    assert!(
        !launch_agent_switched_off(other_key),
        "another key with true"
    );
    assert!(
        !launch_agent_switched_off(&own),
        "own text is not switched off"
    );
}
