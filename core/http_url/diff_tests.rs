//! The reference is the `url` crate: on practical inputs the module must give the same text,
//! and every deliberate difference is pinned in `known_divergences_are_deliberate`.
use super::host::host_to_ascii;
use super::{host, normalize};
use url::Url;

/// What the `url` crate makes of `input`: the serialized URL, if it accepts it.
fn url_normalized(input: &str) -> Option<String> {
    Url::parse(input).ok().map(|u| u.to_string())
}

/// The host the `url` crate extracts from `input`, if it accepts it.
fn url_host(input: &str) -> Option<String> {
    let url = Url::parse(input).ok()?;
    url.host_str().map(str::to_string)
}

fn corpus(schemes: &[&str]) -> Vec<String> {
    let hosts = [
        "example.com",
        "Mixed.Example.COM",
        "x.",
        "b\u{FC}cher.example",
        "93.184.216.34",
        "[::1]",
        "[0:0::ffff:1.2.3.4]",
    ];
    let ports = ["", ":80", ":443", ":8080", ":0080"];
    let paths = ["", "/", "/a/b/"];
    let mut out = Vec::new();
    for scheme in schemes {
        for h in hosts {
            for port in ports {
                for path in paths {
                    out.push(format!("{scheme}://{h}{port}{path}"));
                }
            }
        }
    }
    out
}

#[test]
fn idn_host_matches_url() {
    // Greek capitals, fullwidth Latin and the ideographic full stop go through the same mapping as in `url`.
    for h in [
        "Pr\u{FC}fung.example",
        "M\u{DC}NCHEN.de",
        "\u{FF25}\u{FF38}\u{FF21}\u{FF2D}\u{FF30}\u{FF2C}\u{FF25}.com",
        "\u{4F8B}\u{3048}\u{3002}\u{30C6}\u{30B9}\u{30C8}",
        "\u{3A3}\u{391}\u{3A3}.gr",
        "\u{395}\u{3BB}\u{3BB}\u{3AC}\u{3B4}\u{3B1}.gr",
        "\u{4ED6}\u{4EEC}\u{4E3A}\u{4EC0}\u{4E48}\u{4E0D}\u{8BF4}\u{4E2D}\u{6587}.example",
        "\u{D55C}\u{AD6D}\u{C5B4}.kr",
    ] {
        let url = format!("https://{h}/p");
        let want = url_host(&url).expect("the reference accepts the sample");
        assert!(want.is_ascii(), "reference host must be ASCII: {want}");
        assert_eq!(
            host(&url).as_deref().ok(),
            Some(want.as_str()),
            "host of {h:?}"
        );
        assert_eq!(
            host_to_ascii(h).as_deref().ok(),
            Some(want.as_str()),
            "host_to_ascii of {h:?}"
        );
        assert_eq!(
            normalize(&url).ok(),
            url_normalized(&url),
            "normalize of {h:?}"
        );
    }
}

#[test]
fn normalize_matches_url_on_corpus() {
    for input in corpus(&["http", "HTTPS"]) {
        let want = url_normalized(&input).expect("the reference accepts the corpus");
        assert_eq!(normalize(&input).ok(), Some(want), "input {input:?}");
    }
}

#[test]
fn host_matches_url_on_corpus() {
    for input in corpus(&["http", "HTTPS", "ftp", "ws"]) {
        let want = url_host(&input).expect("the reference accepts the corpus");
        assert_eq!(host(&input).ok(), Some(want), "input {input:?}");
    }
}

/// `url` accepts, the module refuses: `normalize` only.
const NORMALIZE_REFUSES: &[&str] = &[
    // login in an address
    "http://u:p@h/",
    "https://u@h.example/x",
    // path, query and fragment that need percent-encoding
    "http://h/a b",
    "http://h/\u{E9}",
    "http://h/?a b",
    "http://h/?q=\u{E9}",
    "http://h/#a b",
    "http://h/#\u{E9}",
    // backslash, dot segments
    "http://h/a\\b",
    "http://h/a/../b",
    "http://h/./a",
    // tab and line breaks inside
    "http://h/a\tb",
    "http://h/a\nb",
    // schemes other than http and https
    "ftp://h/",
    "ws://h/",
];

/// `url` accepts, the module refuses: `normalize` and `host`.
const BOTH_REFUSE: &[&str] = &[
    // percent sign in a host
    "http://a%41.com/",
    "http://%61.com/",
    // IPv4 that is not in dotted-decimal canonical form
    "http://127.1/",
    "http://0x7f.0.0.1/",
    "http://0177.0.0.1/",
    "http://127.0.0.01/",
    // a tab or a line break inside a host
    "http://ex\tample.com/",
    "http://ex\nample.com/",
];

/// The module accepts, `url` refuses: characters UTS 46 forbids and a broken `xn--` label.
const OURS_ONLY: &[&str] = &[
    "http://a\u{E000}b.com/",
    "http://a\u{378}b.com/",
    "http://xn--a.com/",
];

#[test]
fn known_divergences_are_deliberate() {
    for input in NORMALIZE_REFUSES {
        assert!(url_normalized(input).is_some(), "url must accept {input:?}");
        assert!(normalize(input).is_err(), "normalize must refuse {input:?}");
    }
    for input in BOTH_REFUSE {
        assert!(url_normalized(input).is_some(), "url must accept {input:?}");
        assert!(normalize(input).is_err(), "normalize must refuse {input:?}");
        assert!(host(input).is_err(), "host must refuse {input:?}");
    }
    for input in OURS_ONLY {
        assert!(url_normalized(input).is_none(), "url must refuse {input:?}");
        assert!(normalize(input).is_ok(), "normalize must accept {input:?}");
        assert!(host(input).is_ok(), "host must accept {input:?}");
    }
}

#[test]
fn host_ignores_path_that_normalize_refuses() {
    // The login, the path and the query are dropped by `host`, so for them there is no difference to pin.
    for input in [
        "http://u:p@h.example/a b",
        "http://h.example/a/../b?q=\u{E9}#\u{E9}",
    ] {
        assert_eq!(host(input).ok(), url_host(input), "input {input:?}");
        assert_eq!(host(input).as_deref().ok(), Some("h.example"));
    }
}
