use super::*;

fn view(page: IfaceFilter, proxy: IfaceFilter) -> AccessView {
    AccessView {
        page: ListenAccess::Localnet,
        proxy: ListenAccess::Localhost,
        page_ifaces: page,
        proxy_ifaces: proxy,
    }
}

fn only(names: &[&str]) -> IfaceFilter {
    IfaceFilter::Only(names.iter().map(|s| s.to_string()).collect())
}

#[test]
fn view_from_settings_defaults() {
    let v = AccessView::from_settings(&Settings::default());
    assert_eq!(
        (v.page, v.proxy, v.page_ifaces, v.proxy_ifaces),
        (
            ListenAccess::Localnet,
            ListenAccess::Localhost,
            IfaceFilter::Auto,
            IfaceFilter::Auto
        )
    );
}

#[test]
fn view_from_config_parses_ifaces_shapes() {
    let doc = |web: Value, socks: Value| {
        json!({"web": {"access": "localnet", "ifaces": web},
               "socks": {"access": "localhost", "ifaces": socks}})
    };
    let v = AccessView::from_config(&doc(json!([]), json!(["*"]))).unwrap();
    assert_eq!(
        (v.page_ifaces, v.proxy_ifaces),
        (IfaceFilter::Auto, IfaceFilter::Any)
    );
    let v = AccessView::from_config(&doc(json!(["br-lan", "lo"]), json!([]))).unwrap();
    assert_eq!(v.page_ifaces, only(&["br-lan", "lo"]));
}

#[test]
fn view_from_config_without_field_is_none() {
    let full = json!({"web": {"access": "localnet", "ifaces": []},
                      "socks": {"access": "localhost", "ifaces": []}});
    assert!(AccessView::from_config(&full).is_some());
    for path in [
        ("web", "access"),
        ("web", "ifaces"),
        ("socks", "access"),
        ("socks", "ifaces"),
    ] {
        let mut cut = full.clone();
        cut[path.0].as_object_mut().unwrap().remove(path.1);
        assert!(AccessView::from_config(&cut).is_none(), "{path:?}");
    }
    assert!(AccessView::from_config(&json!({})).is_none());
}

#[test]
fn lines_merge_equal_ifaces() {
    let v = view(only(&["br-lan", "lo"]), only(&["br-lan", "lo"]));
    assert_eq!(
        v.lines(),
        ["page localnet", "proxy localhost", "ifaces br-lan,lo"]
    );
    assert_eq!(
        view(IfaceFilter::Auto, IfaceFilter::Auto).lines()[2],
        "ifaces auto"
    );
    assert_eq!(
        view(IfaceFilter::Any, IfaceFilter::Any).lines()[2],
        "ifaces *"
    );
}

#[test]
fn lines_split_differing_ifaces() {
    let v = view(IfaceFilter::Auto, only(&["br-lan"]));
    assert!(v.ifaces_differ());
    assert_eq!(
        v.lines(),
        [
            "page localnet",
            "proxy localhost",
            "page-ifaces auto",
            "proxy-ifaces br-lan"
        ]
    );
}

#[test]
fn json_has_four_paths_in_config_shape() {
    let v = view(IfaceFilter::Auto, IfaceFilter::Any);
    let expected = json!({
        "web": {"access": "localnet", "ifaces": []},
        "socks": {"access": "localhost", "ifaces": ["*"]},
    });
    assert_eq!(v.json(), expected);
    assert_eq!(AccessView::from_config(&v.json()), Some(v));
}
