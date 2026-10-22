//! Tests of `text_style`: an offline item is gray, regardless of the "chosen" mark.

use super::*;

mod text_style {
    use super::*;

    #[test]
    fn offline_item_is_secondary() {
        let item = ExitItem {
            id_full: "bb22full".to_string(),
            label: "Sedge (offline)".to_string(),
            checked: false,
            online: false,
        };
        assert_eq!(text_style(&item), TextStyle::Secondary);
    }

    #[test]
    fn selected_offline_item_is_secondary() {
        let item = ExitItem {
            id_full: "bb22full".to_string(),
            label: "Sedge (offline)".to_string(),
            checked: true,
            online: false,
        };
        assert_eq!(text_style(&item), TextStyle::Secondary);
    }

    #[test]
    fn online_item_is_default() {
        let item = ExitItem {
            id_full: "aa11full".to_string(),
            label: "Daisy".to_string(),
            checked: false,
            online: true,
        };
        assert_eq!(text_style(&item), TextStyle::Default);
    }
}
