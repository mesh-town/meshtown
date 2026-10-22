use super::IfaceFilter;

fn v(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

#[test]
fn from_names_three_states() {
    assert_eq!(IfaceFilter::from_names(v(&[])), Some(IfaceFilter::Auto));
    assert_eq!(IfaceFilter::from_names(v(&["*"])), Some(IfaceFilter::Any));
    assert_eq!(
        IfaceFilter::from_names(v(&["br-lan", "wlan0"])),
        Some(IfaceFilter::Only(v(&["br-lan", "wlan0"])))
    );
}

#[test]
fn from_names_rejects_bad_lists() {
    assert_eq!(IfaceFilter::from_names(v(&["*", "br-lan"])), None);
    assert_eq!(IfaceFilter::from_names(v(&["br-lan", "br-lan"])), None);
    assert_eq!(IfaceFilter::from_names(v(&[""])), None);
    let many: Vec<String> = (0..65).map(|i| format!("if{i}")).collect();
    assert_eq!(IfaceFilter::from_names(many), None);
    assert_eq!(IfaceFilter::from_names(vec!["a".repeat(65)]), None);
    assert_eq!(IfaceFilter::from_names(v(&["a\nb"])), None);
}

#[test]
fn from_names_allows_space_in_name() {
    assert_eq!(
        IfaceFilter::from_names(v(&["Ethernet 2"])),
        Some(IfaceFilter::Only(v(&["Ethernet 2"])))
    );
}

#[test]
fn parts_and_to_state() {
    assert_eq!(IfaceFilter::Auto.parts(), ("auto", vec![]));
    assert_eq!(IfaceFilter::Any.parts(), ("any", v(&["*"])));
    let only = IfaceFilter::Only(v(&["br-lan"]));
    assert_eq!(only.parts(), ("list", v(&["br-lan"])));
    assert_eq!(IfaceFilter::Auto.to_state(), None);
    assert_eq!(IfaceFilter::Any.to_state(), Some(v(&["*"])));
    assert_eq!(only.to_state(), Some(v(&["br-lan"])));
}

#[test]
fn from_state_cases_and_roundtrip() {
    assert_eq!(IfaceFilter::from_state(None), IfaceFilter::Auto);
    assert_eq!(IfaceFilter::from_state(Some(v(&[]))), IfaceFilter::Auto);
    assert_eq!(
        IfaceFilter::from_state(Some(v(&["*", "x"]))),
        IfaceFilter::Auto
    );
    for f in [
        IfaceFilter::Auto,
        IfaceFilter::Any,
        IfaceFilter::Only(v(&["a", "b"])),
    ] {
        assert_eq!(IfaceFilter::from_state(f.to_state()), f);
    }
}
