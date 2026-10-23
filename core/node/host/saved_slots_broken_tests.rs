//! Refused saved slots: the unusable folders are listed as broken, the ones that only found no room are not.

use super::*;

impl Fixture {
    /// A slot folder whose `config.json` holds `text`.
    fn add_raw_slot(&mut self, text: &str) -> NetSlotId {
        let id = new_slot_id(&self.main_path()).expect("a free slot id");
        let path = net_slot_path(&self.main_path(), &id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
        id
    }

    /// A well-formed slot file without a network key.
    fn add_keyless_slot(&mut self) -> NetSlotId {
        let id = new_slot_id(&self.main_path()).expect("a free slot id");
        let path = net_slot_path(&self.main_path(), &id).unwrap();
        let secret = NodeSecret::from_bytes(&[self.seed; 32]);
        let net = NetSettings::default();
        save_net_file(&path, None, NOW, CREATED - 1, &secret, &net).expect("write the slot file");
        id
    }
}

#[tokio::test]
async fn unusable_folders_are_broken_and_the_others_are_not() {
    let mut fx = setup(60);
    let keyless = fx.add_keyless_slot();
    let invalid = fx.add_raw_slot("{ this is not json");
    let unreadable = fx.add_raw_slot("{}");
    make_unreadable(&fx, &unreadable);
    let main_network = fx.add_slot(0, CREATED);
    let good = fx.add_slots(MAX_NETS);
    fx.when_started();
    let mut rest = vec![invalid, unreadable];
    rest.sort_by_key(|id| id.to_string());
    let want: Vec<NetSlotId> = std::iter::once(keyless).chain(rest).collect();
    assert_eq!(fx.slots.broken(), want);
    assert!(fx.not_started_line(&main_network).is_some());
    assert!(fx.not_started_line(good.last().unwrap()).is_some());
    for id in want.iter().chain([&main_network, good.last().unwrap()]) {
        let path = net_slot_path(&fx.main_path(), id).unwrap();
        assert!(path.exists(), "the folder of {id} is kept");
    }
    fx.stop().await;
}

#[cfg(unix)]
fn make_unreadable(fx: &Fixture, id: &NetSlotId) {
    use std::os::unix::fs::PermissionsExt;
    let path = net_slot_path(&fx.main_path(), id).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
}

#[cfg(not(unix))]
fn make_unreadable(_fx: &Fixture, _id: &NetSlotId) {}
