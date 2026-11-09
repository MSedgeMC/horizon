//! A station's slot changes: its result and, for the anvil, the rename box
//! follow the input slots.
use super::super::Game;
use super::{Kind, anvil};
use horizon_client::stations::hover_name;

impl Game<'_> {
    /// Recompute after slot changes; the anvil box follows its input item.
    pub(super) fn station_after_input(&mut self) {
        let Some(s) = self.station.as_mut() else {
            return;
        };
        if s.kind == Kind::Anvil && s.inputs[0] != s.anvil_last {
            // AnvilScreen.slotChanged: the box follows its input item.
            s.anvil_last = s.inputs[0].clone();
            let name = s.inputs[0]
                .as_ref()
                .map(|i| hover_name(&self.pack.names, i))
                .unwrap_or_default();
            anvil::input_changed(s, name);
        }
        self.station_changed();
        // AnvilScreen.slotChanged set the box's value, which runs the
        // responder: the menu takes the name and a changed one goes to the
        // server (a named item keeps its name and cost there).
        self.remote_anvil_rename();
    }
}
