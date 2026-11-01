//! The menus players of a hosted world opened and closed, as the mods hear
//! them (NeoForge's `PlayerContainerEvent.Open`/`Close`). The hub records
//! each menu it makes a guest's and each it drops; the game tells the mods
//! once the hub is back in its hands, with no player's state swapped in.
use super::Game;
use horizon_client::lan::menu::PlayerContainerEvent;

impl Game<'_> {
    /// The menu changes the hub collected, each told to the mods in order.
    pub(super) fn run_player_container_events(&mut self) {
        loop {
            let Some(host) = self.lan.as_mut() else {
                return;
            };
            if host.hub.player_container_events.is_empty() {
                return;
            }
            let PlayerContainerEvent {
                open,
                player,
                name,
                dimension,
                menu,
                pos,
            } = host.hub.player_container_events.remove(0);
            if open {
                self.native_player_container_open(player, &name, &dimension, &menu, pos);
            } else {
                self.native_player_container_close(player, &name, &dimension, &menu, pos);
            }
        }
    }
}
