//! Tests of the bridge from the UDP pipe crate's log to the `[port]` journal: cutting the prefix, dropping
//! the lines of the session itself, joining the pieces of a line.

use std::io::Write;

use crate::node::output::{log_page, log_summary};

use super::*;

const STAMP: &str = "2001-02-03T04:05:06Z";

mod journal_text_of_a_line {
    use super::*;

    #[test]
    fn cuts_the_time_and_the_level() {
        let line = format!("{STAMP} [Holesail] [WARN] udp socket closed");
        assert_eq!(journal_text(&line), Some("udp socket closed"));
    }

    #[test]
    fn cuts_the_prefix_of_an_error_line_too() {
        let line = format!("{STAMP} [Holesail] [ERROR] send failed: broken pipe");
        assert_eq!(journal_text(&line), Some("send failed: broken pipe"));
    }

    #[test]
    fn drops_the_line_of_the_session_itself() {
        let line = format!("{STAMP} [Holesail] [ERROR] udp session: idle");
        assert_eq!(journal_text(&line), None);
    }

    #[test]
    fn drops_an_empty_line() {
        assert_eq!(journal_text(""), None);
    }
}

mod writes_into_the_journal {
    use super::*;

    #[test]
    fn two_pieces_and_a_newline_make_one_line() {
        // given: a bridge and a journal mark
        let (mut bridge, seq) = (JournalLines::default(), journal_mark());

        // when: the line arrives in three writes, the last one ends it
        bridge
            .write_all(format!("{STAMP} [Holesail] [WARN] piece-").as_bytes())
            .unwrap();
        bridge.write_all(b"one and ").unwrap();
        bridge.write_all(b"two\n").unwrap();

        // then: one line with the whole text
        let lines = port_lines_since(seq);
        assert_eq!(
            lines
                .iter()
                .filter(|t| t.contains("piece-one and two"))
                .count(),
            1,
            "{lines:?}"
        );
    }

    #[test]
    fn an_unfinished_line_is_not_written_yet() {
        // given: a bridge and a journal mark
        let (mut bridge, seq) = (JournalLines::default(), journal_mark());

        // when: a line without a newline arrives
        bridge
            .write_all(format!("{STAMP} [Holesail] [WARN] unfinished-line").as_bytes())
            .unwrap();

        // then: the journal has nothing of it
        let lines = port_lines_since(seq);
        assert!(
            !lines.iter().any(|t| t.contains("unfinished-line")),
            "{lines:?}"
        );
    }

    #[test]
    fn two_lines_in_one_write_make_two_entries() {
        let (mut bridge, seq) = (JournalLines::default(), journal_mark());
        let both = format!(
            "{STAMP} [Holesail] [WARN] first-of-two\n{STAMP} [Holesail] [WARN] second-of-two\n"
        );

        bridge.write_all(both.as_bytes()).unwrap();

        let lines = port_lines_since(seq);
        assert!(lines.iter().any(|t| t == "first-of-two"), "{lines:?}");
        assert!(lines.iter().any(|t| t == "second-of-two"), "{lines:?}");
    }

    #[test]
    fn the_line_of_the_session_itself_is_not_written() {
        let (mut bridge, seq) = (JournalLines::default(), journal_mark());

        bridge
            .write_all(format!("{STAMP} [Holesail] [ERROR] udp session: bridge-mark\n").as_bytes())
            .unwrap();

        let lines = port_lines_since(seq);
        assert!(
            !lines.iter().any(|t| t.contains("bridge-mark")),
            "{lines:?}"
        );
    }

    #[test]
    fn write_reports_every_byte_as_taken() {
        let mut bridge = JournalLines::default();
        let data = format!("{STAMP} [Holesail] [WARN] counted\n");
        assert_eq!(bridge.write(data.as_bytes()).unwrap(), data.len());
    }
}

fn journal_mark() -> u64 {
    log_summary(0).last_seq
}

/// Texts of the `port` journal lines written after `seq`.
fn port_lines_since(seq: u64) -> Vec<String> {
    log_page(seq, 4000)
        .lines
        .into_iter()
        .filter(|l| l.tag == "port")
        .map(|l| l.text)
        .collect()
}
