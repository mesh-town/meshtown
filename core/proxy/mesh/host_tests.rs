use std::net::Ipv4Addr;

use super::*;
use crate::proxy::socks5::Target;

mod parse {
    use super::*;

    mod our_host {
        use super::*;

        #[test]
        fn plain_name_gives_its_label() {
            assert_eq!(label_of("box.meshtown"), Some("box".to_string()));
        }

        #[test]
        fn one_trailing_dot_is_ignored() {
            assert_eq!(label_of("box.meshtown."), Some("box".to_string()));
        }

        #[test]
        fn many_trailing_dots_are_ignored() {
            assert_eq!(label_of("box.meshtown.."), Some("box".to_string()));
        }

        #[test]
        fn nested_label_is_kept_as_is() {
            assert_eq!(label_of("a.b.meshtown"), Some("a.b".to_string()));
        }

        #[test]
        fn bare_suffix_has_empty_label() {
            assert_eq!(label_of("meshtown"), Some(String::new()));
        }

        #[test]
        fn leading_dot_suffix_has_empty_label() {
            assert_eq!(label_of(".meshtown"), Some(String::new()));
        }
    }

    mod foreign_host {
        use super::*;

        #[test]
        fn suffix_as_first_label_of_a_real_domain_is_not_ours() {
            assert_eq!(label_of("meshtown.com"), None);
        }

        #[test]
        fn suffix_followed_by_other_labels_is_not_ours() {
            assert_eq!(label_of("box.meshtown.evil.com"), None);
        }

        #[test]
        fn suffix_glued_to_a_longer_label_is_not_ours() {
            assert_eq!(label_of("xmeshtown"), None);
        }

        #[test]
        fn ip_target_is_not_ours() {
            let target = Target::Ip(Ipv4Addr::new(10, 0, 0, 1).into());
            assert!(
                MeshHost::parse(&target).is_none(),
                "an IP target never names a mesh device"
            );
        }
    }
}

mod mesh_url {
    use super::*;

    #[test]
    fn port_80_is_plain_http_without_port() {
        assert_eq!(mesh_url("box", 80), "http://box.meshtown");
    }

    #[test]
    fn other_port_is_appended() {
        assert_eq!(mesh_url("box", 8080), "http://box.meshtown:8080");
    }

    #[test]
    fn port_443_is_https_without_port() {
        assert_eq!(mesh_url("box", 443), "https://box.meshtown");
    }

    #[test]
    fn address_with_tail_is_kept_whole() {
        assert_eq!(mesh_url("box-a1b2", 80), "http://box-a1b2.meshtown");
    }
}

mod host_url {
    use super::*;

    #[test]
    fn port_80_is_plain_http_without_port() {
        assert_eq!(host_url("box.meshtown", 80), "http://box.meshtown");
    }

    #[test]
    fn other_port_is_appended() {
        assert_eq!(host_url("box.meshtown", 8080), "http://box.meshtown:8080");
    }

    #[test]
    fn port_443_is_https_without_port() {
        assert_eq!(host_url("box.meshtown", 443), "https://box.meshtown");
    }

    #[test]
    fn gives_the_same_link_as_mesh_url_for_the_same_device() {
        for port in [80, 443, 22, 8080] {
            assert_eq!(
                host_url("box-a1b2.meshtown", port),
                mesh_url("box-a1b2", port),
                "port {port}"
            );
        }
    }
}

fn label_of(host: &str) -> Option<String> {
    MeshHost::parse(&Target::Domain(host.to_string())).map(|h| h.label)
}
