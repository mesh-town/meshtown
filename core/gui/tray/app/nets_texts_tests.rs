use super::*;

#[test]
fn deleted_network_note_quotes_the_name() {
    assert_eq!(
        net_deleted_text("work"),
        "\"work\" was deleted by an admin — open Members…"
    );
}
