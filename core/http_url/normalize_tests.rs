use super::*;
use crate::http_url::host;

#[test]
fn normalize_refuses_non_http_and_empty_host() {
    for bad in [
        "ftp://x",
        "x.org",
        "https://",
        "http://",
        "",
        "   ",
        "mailto:x@y.org",
    ] {
        assert!(normalize(bad).is_err(), "should be refused: {bad:?}");
    }
}

#[test]
fn host_ignores_userinfo_port_path_query() {
    assert_eq!(host("http://u:p@h:8080/p?q#f").unwrap(), "h");
    assert_eq!(host("ftp://user@Example.COM:21/x").unwrap(), "example.com");
    assert_eq!(host("https://h.example?q=1").unwrap(), "h.example");
    assert_eq!(host("https://h.example#frag").unwrap(), "h.example");
}

mod canonical_form {
    use super::*;

    #[test]
    fn lowercases_scheme_and_host_and_adds_root_path() {
        assert_eq!(
            normalize("HTTPS://Relay.Example.COM").unwrap(),
            "https://relay.example.com/"
        );
    }

    #[test]
    fn keeps_trailing_dot_of_host() {
        assert_eq!(
            normalize("https://relay.example./").unwrap(),
            "https://relay.example./"
        );
    }

    #[test]
    fn drops_default_port_including_leading_zeros() {
        assert_eq!(normalize("http://h:80/").unwrap(), "http://h/");
        assert_eq!(normalize("https://h:443").unwrap(), "https://h/");
        assert_eq!(normalize("http://h:0080/").unwrap(), "http://h/");
    }

    #[test]
    fn keeps_non_default_port_and_path() {
        assert_eq!(
            normalize("http://h:8080/a/b/").unwrap(),
            "http://h:8080/a/b/"
        );
        assert_eq!(normalize("http://h:443/").unwrap(), "http://h:443/");
    }

    #[test]
    fn trims_blanks_around_the_input() {
        assert_eq!(normalize("  http://h/ \n").unwrap(), "http://h/");
    }

    #[test]
    fn converts_idn_host_to_punycode() {
        assert_eq!(
            normalize("https://b\u{FC}cher.example/x").unwrap(),
            "https://xn--bcher-kva.example/x"
        );
    }
}
