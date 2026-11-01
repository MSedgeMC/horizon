//! `ChunkMap.TrackedEntity`'s pairings as the mods hear them: the entities
//! each player of a hosted world started and stopped tracking (NeoForge's
//! `PlayerEvent.StartTracking`/`StopTracking`). The hub's per-client `Tracker`
//! holds the set of entities a client is paired with and records what changes
//! it; the game tells the mods once a tick, with no player's state swapped in.
use super::Game;
use horizon_client::lan::{entity::TrackingChange, hub::TrackingEvent};

impl Game<'_> {
    /// The pairing changes the hub collected, each told to the mods with the
    /// hub back in `self.lan`.
    pub(super) fn run_tracking_events(&mut self) {
        loop {
            let Some(host) = self.lan.as_mut() else {
                return;
            };
            if host.hub.tracking_events.is_empty() {
                return;
            }
            let TrackingEvent {
                start,
                player,
                name,
                entity,
            } = host.hub.tracking_events.remove(0);
            let TrackingChange {
                entity: id,
                uuid,
                kind,
            } = entity;
            if start {
                self.native_player_start_tracking(player, &name, id, uuid, &kind);
            } else {
                self.native_player_stop_tracking(player, &name, id, uuid, &kind);
            }
        }
    }
}
