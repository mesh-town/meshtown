use super::*;

#[test]
fn deleted_network_note_quotes_the_name() {
    assert_eq!(
        net_deleted_text("work"),
        "\"work\" was deleted by an admin — open Members…"
    );
}

#[test]
fn deleted_network_note_without_a_name_has_a_fallback() {
    let fallback = "A network was deleted by an admin — open Members…";
    assert_eq!(net_deleted_text(""), fallback);
    assert_eq!(net_deleted_text("  "), fallback);
}
