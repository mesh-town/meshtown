//! `PUT /api/config` and the `local` ports of `ports.opened`: a port that is new in the list and already opened in
//! another network of the node is refused. The harness is from `mod_tests.rs` via `use super::*;`.

use super::*;
use crate::ports_sharing::Proto;
use crate::transport::net::NodeSecret;
use crate::web_gui::host_control::OpenedLocal;

mod a_local_new_in_the_list {
    use super::*;

    mod opened_in_another_network {
        use super::*;

        #[test]
        fn is_422_on_the_ports_opened_field() {
            // given
            let mut f = setup();
            // when
            let (status, v) = f.when_put(opens(&[9000]), &taken_in_work(&[9000]));
            // then
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {v}");
            assert_eq!(v["error"], "invalid");
            assert_eq!(v["fields"][0]["path"], "ports.opened");
            assert_eq!(
                v["fields"][0]["message"],
                "local port 9000 is already opened in network \"work\""
            );
        }

        #[test]
        fn leaves_the_file_unchanged() {
            // given
            let mut f = setup();
            let before = f.file();
            // when
            let (status, _) = f.when_put(opens(&[9000]), &taken_in_work(&[9000]));
            // then
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(f.file(), before, "the settings file");
            assert!(f.state.file_settings.net.ports_opened.is_empty());
        }

        #[test]
        fn gives_one_error_for_the_first_such_local() {
            // given: both new locals are taken
            let mut f = setup();
            // when
            let (_, v) = f.when_put(opens(&[9001, 9000]), &taken_in_work(&[9000, 9001]));
            // then
            assert_eq!(v["fields"].as_array().unwrap().len(), 1, "body: {v}");
            assert_eq!(
                v["fields"][0]["message"],
                "local port 9001 is already opened in network \"work\""
            );
        }
    }

    mod nobody_else_opens {
        use super::*;

        #[test]
        fn is_accepted_when_another_local_is_taken() {
            // given
            let mut f = setup();
            // when
            let (status, v) = f.when_put(opens(&[9001]), &taken_in_work(&[9000]));
            // then
            assert_eq!(status, StatusCode::OK, "body: {v}");
            assert_eq!(f.state.file_settings.net.ports_opened.len(), 1);
        }

        #[test]
        fn is_accepted_when_nothing_is_taken() {
            // given
            let mut f = setup();
            // when
            let (status, v) = f.when_put(opens(&[9000]), &[]);
            // then
            assert_eq!(status, StatusCode::OK, "body: {v}");
        }
    }
}

mod a_local_already_in_the_file {
    use super::*;

    #[test]
    fn does_not_stop_other_fields_from_being_saved_when_another_network_opens_it() {
        // given: the file of this slot already holds `local 9000`, and another network opens it too
        let mut f = setup();
        let (status, v) = f.when_put(opens(&[9000]), &[]);
        assert_eq!(status, StatusCode::OK, "the initial write: {v}");
        // when: a PUT changes another field and keeps the list as it is
        let mut config = opens(&[9000]);
        config["checks"] = serde_json::json!({"interval_sec": 600});
        let (status, v) = f.when_put(config, &taken_in_work(&[9000]));
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
        assert_eq!(f.state.file_settings.net.check_interval_sec, 600);
    }
}

mod the_protocol_of_the_local {
    use super::*;

    #[test]
    fn udp_local_taken_in_another_network_is_422() {
        // given: another network opens UDP 9000
        let mut f = setup();
        // when
        let (status, v) = f.when_put(opens_over("udp", &[9000]), &taken(Proto::Udp, &[9000]));
        // then
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {v}");
        assert_eq!(v["fields"][0]["path"], "ports.opened");
        assert_eq!(
            v["fields"][0]["message"],
            "local UDP port 9000 is already opened in network \"work\""
        );
    }

    #[test]
    fn udp_clash_leaves_the_file_unchanged() {
        // given
        let mut f = setup();
        let before = f.file();
        // when
        let (status, _) = f.when_put(opens_over("udp", &[9000]), &taken(Proto::Udp, &[9000]));
        // then
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(f.file(), before, "the settings file");
    }

    #[test]
    fn tcp_clash_keeps_the_tcp_text_without_a_protocol_word() {
        // given: another network opens TCP 9000, and the PUT names the protocol explicitly
        let mut f = setup();
        // when
        let (_, v) = f.when_put(opens_over("tcp", &[9000]), &taken(Proto::Tcp, &[9000]));
        // then
        assert_eq!(
            v["fields"][0]["message"],
            "local port 9000 is already opened in network \"work\""
        );
    }

    #[test]
    fn tcp_and_udp_on_one_local_do_not_clash() {
        // given: another network opens UDP 9000
        let mut f = setup();
        // when: this one opens TCP 9000
        let (status, v) = f.when_put(opens(&[9000]), &taken(Proto::Udp, &[9000]));
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
    }

    #[test]
    fn udp_over_a_tcp_local_of_another_network_is_accepted() {
        // given: another network opens TCP 9000
        let mut f = setup();
        // when: this one opens UDP 9000
        let (status, v) = f.when_put(opens_over("udp", &[9000]), &taken(Proto::Tcp, &[9000]));
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
    }

    #[test]
    fn udp_entry_is_saved_with_its_protocol() {
        // given
        let mut f = setup();
        // when
        let (status, v) = f.when_put(opens_over("udp", &[9000]), &[]);
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
        assert_eq!(f.saved_opened()[0]["proto"], "udp");
        assert_eq!(f.state.file_settings.net.ports_opened[0].proto, Proto::Udp);
    }

    #[test]
    fn explicit_tcp_is_accepted_and_not_written() {
        // given
        let mut f = setup();
        // when
        let (status, v) = f.when_put(opens_over("tcp", &[9000]), &[]);
        // then
        assert_eq!(status, StatusCode::OK, "body: {v}");
        assert!(
            f.saved_opened()[0].get("proto").is_none(),
            "TCP entries carry no proto"
        );
    }
}

/// One slot's settings over a file in a fresh directory.
struct Fixture {
    dir: PathBuf,
    state: ConfigState,
    node: StubNode,
}

impl Fixture {
    /// A `PUT` of `config` at the current revision, with the locals the other networks of the node hold.
    fn when_put(&mut self, config: Value, taken: &[OpenedLocal]) -> (StatusCode, Value) {
        let body = put_body(self.state.file_report.rev, config);
        let (resp, _) = self.state.put_keeping(&body, &self.node, None, taken);
        body_json(resp)
    }

    fn file(&self) -> Vec<u8> {
        std::fs::read(self.dir.join("config.json")).unwrap_or_default()
    }

    /// `config.ports.opened` as written to the settings file.
    fn saved_opened(&self) -> Vec<Value> {
        let doc: Value = serde_json::from_slice(&self.file()).expect("the file is JSON");
        doc["config"]["ports"]["opened"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        cleanup(&self.dir);
    }
}

fn setup() -> Fixture {
    let dir = temp_dir();
    let state = new_state(&dir);
    Fixture {
        dir,
        state,
        node: StubNode::new(),
    }
}

/// A config part that opens one port of a peer on each of the given local ports.
fn opens(locals: &[u16]) -> Value {
    let node = NodeSecret::from_bytes(&[7u8; 32]).peer_id().to_string();
    let list: Vec<Value> = locals
        .iter()
        .enumerate()
        .map(|(i, local)| serde_json::json!({"node": node, "port": 80 + i, "local": local}))
        .collect();
    serde_json::json!({"ports": {"opened": list}})
}

fn taken_in_work(locals: &[u16]) -> Vec<OpenedLocal> {
    let work = |local: &u16| OpenedLocal {
        local: *local,
        proto: Proto::Tcp,
        net: "work".to_string(),
    };
    locals.iter().map(work).collect()
}

/// Like `opens`, with the protocol named in every entry.
fn opens_over(proto: &str, locals: &[u16]) -> Value {
    let mut config = opens(locals);
    for entry in config["ports"]["opened"]
        .as_array_mut()
        .into_iter()
        .flatten()
    {
        entry["proto"] = Value::from(proto);
    }
    config
}

fn taken(proto: Proto, locals: &[u16]) -> Vec<OpenedLocal> {
    taken_in_work(locals)
        .into_iter()
        .map(|t| OpenedLocal { proto, ..t })
        .collect()
}
