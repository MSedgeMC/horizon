//! A station's slot changes: its result and, for the anvil, the rename box
//! follow the input slots.
use super::super::Game;

use horizon_client::stations::hover_name;
use horizon_protocol::packets::play::{self, Serverbound as S};

impl Game<'_> {
    /// Recompute after slot changes; the anvil box follows its input item and a
    /// changed name goes to the server, as `AnvilScreen.slotChanged` does
    /// through `EditBox.setValue` and its responder.
    pub(super) fn station_after_input(&mut self) {
        let Some(input) = self.station.as_ref().map(|s| s.inputs[0].clone()) else {
            return;
        };
        let name = input
            .as_ref()
            .map(|i| hover_name(&self.pack.names, i))
            .unwrap_or_default();
        let sent = self.station.as_mut().and_then(|s| s.after_input(name));
        self.station_changed();
        // `AnvilScreen.onNameChanged`: a name the server does not have yet.
        if let Some(name) = sent {
            self.send(S::RenameItem(play::RenameItem { name }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Kind, Station};
    use horizon_client::items::ItemStack;

    /// `AnvilScreen.slotChanged`: the box follows the input item, and the name
    /// it shows is what `AnvilScreen.onNameChanged` hands the menu — only a
    /// name the server does not have goes out, and the box's own name is blank.
    /// This is `Station::after_input`, the function `station_after_input` calls
    /// for every slot change.
    #[test]
    fn a_slot_change_sends_only_a_changed_rename() {
        let mut station = Station::new_kind(Kind::Anvil, [0; 3]);
        station.inputs[0] = Some(ItemStack::new("minecraft:iron_sword", 1));
        assert_eq!(
            station.after_input("Sword".into()),
            Some("Sword".into()),
            "the item's own name goes blank, so the first change is sent"
        );
        assert_eq!(
            station.after_input("Sword".into()),
            None,
            "the server already has that name"
        );
        assert_eq!(
            station.after_input(String::new()),
            Some(String::new()),
            "blanking it is a change of its own"
        );
        assert_eq!(station.after_input(String::new()), None);
        // No input item: nothing to rename.
        station.inputs[0] = None;
        assert_eq!(station.after_input("Sword".into()), None);
        // And a station that is not the anvil never sends a rename.
        let mut table = Station::new_kind(Kind::Enchanting, [0; 3]);
        table.inputs[0] = Some(ItemStack::new("minecraft:diamond_sword", 1));
        assert_eq!(table.after_input("Sword".into()), None);
    }
}
