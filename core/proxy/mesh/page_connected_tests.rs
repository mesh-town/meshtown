use super::tests::*;
use super::*;

fn page(devices: Vec<SharedDevice>) -> MeshPage {
    make(PageKind::Connected { devices })
}

fn device(name: &str, online: bool, ports: &[(u16, &str)]) -> SharedDevice {
    SharedDevice {
        name: name.to_string(),
        online,
        open: open_ports(ports),
    }
}

mod connected {
    use super::*;

    #[test]
    fn status_is_200() {
        assert_status(&page(vec![]), "HTTP/1.1 200 OK");
    }

    #[test]
    fn lists_device_ports_as_links() {
        let body = body_of(&page(vec![device("Box", false, &[(3000, "web")])]));
        assert!(body.contains("<strong>Box</strong>"), "{body}");
        assert!(
            body.contains("<span class=\"badge\">offline</span>"),
            "{body}"
        );
        assert!(
            body.contains("<a href=\"http://router.meshtown:3000\">"),
            "{body}"
        );
        assert!(
            body.contains("This browser is connected to Mesh Town"),
            "{body}"
        );
        assert!(
            body.contains("open through Mesh Town on Cafe Laptop."),
            "{body}"
        );
    }

    #[test]
    fn empty_says_nobody_shared() {
        let body = body_of(&page(vec![]));
        assert!(
            body.contains("No member has shared a port with you yet."),
            "{body}"
        );
    }

    #[test]
    fn escapes_device_and_port_names() {
        let body = body_of(&page(vec![device("<b>Box", true, &[(80, "<b>web")])]));
        assert!(!body.contains("<b>"), "{body}");
        assert!(body.contains("&lt;b&gt;Box"), "{body}");
        assert!(body.contains("&lt;b&gt;web"), "{body}");
    }

    #[test]
    fn no_who_line() {
        let body = body_of(&page(vec![]));
        assert!(!body.contains("not from"), "{body}");
    }
}
