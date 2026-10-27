//! Tests for the proxy auto-config file (`pac.rs`): request recognition, route string, script and reply.

use super::*;

mod pac_request {
    use super::*;

    #[test]
    fn get_on_pac_path_is_full_request() {
        // given
        let (method, path) = ("GET", "/proxy.pac");
        // when
        let ask = pac_request(method, path);
        // then
        assert_eq!(ask, Some(PacRequest { head_only: false }));
    }

    #[test]
    fn head_on_pac_path_is_head_only() {
        let ask = pac_request("HEAD", "/proxy.pac");
        assert_eq!(ask, Some(PacRequest { head_only: true }));
    }

    #[test]
    fn query_tail_is_ignored() {
        let ask = pac_request("GET", "/proxy.pac?x=1");
        assert_eq!(ask, Some(PacRequest { head_only: false }));
    }

    mod not_pac {
        use super::*;

        #[test]
        fn root_path() {
            assert_eq!(pac_request("GET", "/"), None);
        }

        #[test]
        fn wpad_path() {
            assert_eq!(pac_request("GET", "/wpad.dat"), None);
        }

        #[test]
        fn longer_name() {
            assert_eq!(pac_request("GET", "/proxy.pacx"), None);
        }

        #[test]
        fn trailing_slash() {
            assert_eq!(pac_request("GET", "/proxy.pac/"), None);
        }

        #[test]
        fn upper_case_path() {
            assert_eq!(pac_request("GET", "/PROXY.PAC"), None);
        }

        #[test]
        fn post_method() {
            assert_eq!(pac_request("POST", "/proxy.pac"), None);
        }

        #[test]
        fn lower_case_method() {
            assert_eq!(pac_request("get", "/proxy.pac"), None);
        }

        #[test]
        fn absolute_form() {
            assert_eq!(pac_request("GET", "http://127.0.0.1:1080/proxy.pac"), None);
        }
    }
}

mod route_line {
    use super::*;

    #[test]
    fn goes_socks5_then_http_then_direct_on_actual_port() {
        // given
        let proxy = addr("127.0.0.1:41234");
        // when
        let line = pac_proxy_list(proxy);
        // then
        assert_eq!(line, "SOCKS5 127.0.0.1:41234; PROXY 127.0.0.1:41234");
    }

    #[test]
    fn route_list_ends_with_direct_unless_strict() {
        // given: port 41234; when: not strict
        let line = pac_route_list(addr("127.0.0.1:41234"), false);
        // then
        assert_eq!(
            line,
            "SOCKS5 127.0.0.1:41234; PROXY 127.0.0.1:41234; DIRECT"
        );
    }

    #[test]
    fn strict_route_list_has_no_direct() {
        let line = pac_route_list(addr("127.0.0.1:41234"), true);
        assert_eq!(line, "SOCKS5 127.0.0.1:41234; PROXY 127.0.0.1:41234");
    }

    #[test]
    fn script_keeps_direct_for_ordinary_sites_only() {
        // given: both lists in the script
        let proxy = addr("127.0.0.1:41234");
        let script = pac_script(&pac_route_list(proxy, false), &pac_proxy_list(proxy));
        // then: the route return has the tail, the mesh branch does not
        assert!(
            script.contains("return \"SOCKS5 127.0.0.1:41234; PROXY 127.0.0.1:41234; DIRECT\";")
        );
        assert!(script.contains("return \"SOCKS5 127.0.0.1:41234; PROXY 127.0.0.1:41234\";"));
    }

    #[test]
    fn strict_flag_round_trips() {
        // given/when: the flag is set; then: it reads back, and goes back to false at once
        set_pac_strict(true);
        let on = pac_strict();
        set_pac_strict(false);
        assert!(on);
        assert!(!pac_strict());
    }

    #[test]
    fn lan_address_goes_into_line_as_is() {
        // `localnet` mode: the PAC for a network device carries the address the connection arrived on
        let line = pac_proxy_list(addr("192.168.1.5:1080"));
        assert_eq!(line, "SOCKS5 192.168.1.5:1080; PROXY 192.168.1.5:1080");
    }
}

mod pac_script {
    use super::*;

    #[test]
    fn puts_route_into_return_and_leaves_no_marker() {
        // given
        let route = "SOCKS5 127.0.0.1:1080; PROXY 127.0.0.1:1080";
        // when
        let script = pac_script(route, route);
        // then
        assert!(
            script.contains(&format!("return \"{route}\";")),
            "no route string in return:\n{script}"
        );
        assert!(
            !script.contains("__ROUTE__"),
            "the marker is left in the script:\n{script}"
        );
    }

    #[test]
    fn defines_find_proxy_for_url() {
        let script = pac_script("DIRECT", "DIRECT");
        assert!(
            script.contains("function FindProxyForURL(url, host)"),
            "script:\n{script}"
        );
    }

    #[test]
    fn never_resolves_names() {
        // given: the comment in the script header names these functions, so we look only at the code.
        let code = code_without_comments(&pac_script("DIRECT", "DIRECT"));
        // then
        for call in ["dnsResolve", "isInNet", "myIpAddress", "isResolvable"] {
            assert!(!code.contains(call), "the script calls {call}:\n{code}");
        }
    }

    #[test]
    fn is_plain_es5() {
        let code = code_without_comments(&pac_script("DIRECT", "DIRECT"));
        for syntax in ["const ", "let ", "=>", "`"] {
            assert!(
                !code.contains(syntax),
                "the script has {syntax:?} (not ES5):\n{code}"
            );
        }
    }

    #[test]
    fn is_ascii_only() {
        let script = pac_script("DIRECT", "DIRECT");
        assert!(
            script.is_ascii(),
            "without a charset the body is read as Latin-1:\n{script}"
        );
    }

    #[test]
    fn leaves_no_marker_for_route_or_mesh() {
        let script = pac_script("DIRECT", "SOCKS5 a; PROXY a");
        for marker in ["__MESH__", "__ROUTE__"] {
            assert!(
                !script.contains(marker),
                "the marker {marker} is left in the script:\n{script}"
            );
        }
    }

    #[test]
    fn puts_mesh_route_into_return_before_local_check_and_direct() {
        // given
        let script = pac_script("DIRECT", "SOCKS5 a; PROXY a");
        // then
        let mesh = position(&script, "return \"SOCKS5 a; PROXY a\"");
        assert!(
            mesh < position(&script, "isLocalHost("),
            "the mesh route comes after the local check:\n{script}"
        );
        assert!(
            mesh < position(&script, "return \"DIRECT\""),
            "the mesh route comes after the direct route:\n{script}"
        );
    }

    #[test]
    fn bare_mesh_name_goes_to_mesh_route() {
        let script = pac_script("DIRECT", "SOCKS5 a; PROXY a");
        let branch = mesh_branch(&script);
        assert!(
            branch.contains("h == \"meshtown\""),
            "no test for the bare name:\n{branch}"
        );
    }

    #[test]
    fn name_under_mesh_suffix_with_trailing_dot_goes_to_mesh_route() {
        // `box.meshtown.` is normalised (lower case, trailing dots dropped) before the suffix check.
        let script = pac_script("DIRECT", "SOCKS5 a; PROXY a");
        let branch = mesh_branch(&script);
        assert!(
            branch.contains("dnsDomainIs(h, \".meshtown\")"),
            "no test for the suffix:\n{branch}"
        );
        let suffix = position(&script, "dnsDomainIs(h, \".meshtown\")");
        assert!(
            position(&script, ".replace(/\\.+$/, \"\")") < suffix,
            "trailing dots are not dropped before the suffix check:\n{script}"
        );
        assert!(
            position(&script, ".toLowerCase()") < suffix,
            "the host is not lower-cased before the suffix check:\n{script}"
        );
    }
}

mod pac_reply {
    use super::*;

    #[test]
    fn get_has_exact_headers_then_script() {
        // given
        let script = "function FindProxyForURL(url, host) { return \"DIRECT\"; }\n";
        // when
        let (head, body) = split_reply(&pac_reply(script, false));
        // then
        let expected = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close",
            script.len()
        );
        assert_eq!(head, expected);
        assert_eq!(body, script);
    }

    #[test]
    fn content_length_counts_body_bytes() {
        // given: multibyte text — more bytes than characters.
        let script = "// route \u{2014}\n";
        assert!(script.len() > script.chars().count());
        // when
        let (head, body) = split_reply(&pac_reply(script, false));
        // then
        assert_eq!(content_length(&head), body.len());
        assert_eq!(content_length(&head), script.len());
    }

    #[test]
    fn head_keeps_length_and_drops_body() {
        // given
        let script = pac_script(&pac_proxy_list(addr("127.0.0.1:1080")), "DIRECT");
        let (get_head, _) = split_reply(&pac_reply(&script, false));
        // when
        let (head, body) = split_reply(&pac_reply(&script, true));
        // then
        assert_eq!(body, "", "HEAD has no body");
        assert_eq!(head, get_head, "the HEAD headers are the same as GET's");
        assert_eq!(content_length(&head), script.len());
    }
}

mod pac_url {
    use super::*;

    #[test]
    fn points_at_proxy_address_and_pac_path() {
        let url = pac_url(addr("127.0.0.1:1080"));
        assert_eq!(url, "http://127.0.0.1:1080/proxy.pac");
    }
}

fn addr(text: &str) -> SocketAddr {
    text.parse().expect("socket address")
}

/// The script lines without `//` comments.
fn code_without_comments(script: &str) -> String {
    script
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Splits the reply into the head (without `\r\n\r\n`) and the body.
fn split_reply(reply: &[u8]) -> (String, String) {
    let text = String::from_utf8(reply.to_vec()).expect("the reply is UTF-8");
    let (head, body) = text.split_once("\r\n\r\n").expect("no end of the head");
    (head.to_string(), body.to_string())
}

fn content_length(head: &str) -> usize {
    let line = head
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .expect("no Content-Length");
    line.parse().expect("Content-Length is a number")
}

/// Byte position of `needle` in `script`; fails the test with the script when it is missing.
fn position(script: &str, needle: &str) -> usize {
    script
        .find(needle)
        .unwrap_or_else(|| panic!("no {needle:?} in the script:\n{script}"))
}

/// The `if` block of `FindProxyForURL` that tests the mesh names: from the bare-name test up to the first `}`,
/// proven to hold the mesh route as its `return`.
fn mesh_branch(script: &str) -> String {
    let from = position(script, "h == \"meshtown\"");
    let tail = &script[from..];
    let branch = &tail[..tail.find('}').expect("the branch is closed")];
    assert!(
        branch.contains("return \"SOCKS5 a; PROXY a\""),
        "the mesh branch does not return the mesh route:\n{branch}"
    );
    branch.to_string()
}
