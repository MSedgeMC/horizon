//! `LivingEntity.detectEquipmentUpdates` for mobs: what each mob wears or
//! holds, kept from the last tick and compared once a tick after the mobs
//! ticked, so a mod hears a slot take a different item
//! (`native_living_equipment_change`, NeoForge's `LivingEquipmentChangeEvent`).
//! Vanilla runs the same comparison for every living entity it ticks on a
//! server; a player's equips reach `native_on_equip` instead, which also hears
//! the paths that never reach a tick (`/item`).
use super::Game;
use horizon_client::{items::ItemStack, mobs::finalize};
use horizon_world::loot::EQUIPMENT_SLOTS;
use std::collections::HashMap;

/// The slots compared each tick: the `EquipmentSlot` values a mob of Horizon
/// can hold an item in. `body` is left out: no mob species of this pass puts
/// an item there (see `equip::slot_for`), so it would never change.
const SLOTS: [usize; 7] = [
    finalize::MAINHAND,
    finalize::OFFHAND,
    finalize::FEET,
    finalize::LEGS,
    finalize::CHEST,
    finalize::HEAD,
    7,
];

/// `ItemStack.matches` over optional stacks: the same item, components and
/// count, or nothing on both sides.
fn same(a: Option<&ItemStack>, b: Option<&ItemStack>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.count == b.count && a.same_item_same_components(b),
        _ => false,
    }
}

impl Game<'_> {
    /// `LivingEntity.collectEquipmentChanges` for the level's mobs: for each
    /// slot whose item differs from the last tick's, the mods hear it
    /// (`native_living_equipment_change`). A mob new to the level counts from
    /// empty, as vanilla's `lastEquipmentItems` start out, so a mob that
    /// spawns equipped is heard on its first tick. The mobs of a level that
    /// is not in place keep no record and are heard again from empty when it
    /// comes back.
    pub(super) fn detect_mob_equipment_changes(&mut self) {
        let dim = self.dim.id();
        let mut next: HashMap<u32, [Option<ItemStack>; 8]>;
        let mut heard = Vec::new();
        next = HashMap::with_capacity(self.mobs.len());
        for m in &self.mobs {
            let last = self.mob_equipment.get(&m.id);
            let mut now: [Option<ItemStack>; 8] = Default::default();
            for slot in SLOTS {
                let current = finalize::item_stack(m, slot);
                let before = last.and_then(|l| l[slot].as_ref());
                if !same(before, current.as_ref()) {
                    heard.push((
                        m.kind.clone(),
                        m.id,
                        EQUIPMENT_SLOTS[slot],
                        before.cloned(),
                        current.clone(),
                    ));
                }
                now[slot] = current;
            }
            next.insert(m.id, now);
        }
        self.mob_equipment = next;
        for (kind, id, slot, from, to) in heard {
            self.native_living_equipment_change(
                dim,
                id,
                &kind.id,
                slot,
                from.as_ref(),
                to.as_ref(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item: &str, count: u32) -> ItemStack {
        ItemStack::new(item, count)
    }

    /// `ItemStack.matches` over optional stacks: nothing to nothing is no
    /// change, and the item, its count, its damage and its components all
    /// count as the change.
    #[test]
    fn a_slot_changes_when_its_stack_is_not_the_same_one() {
        let arrow = Some(stack("minecraft:arrow", 1));
        assert!(same(None, None));
        assert!(same(arrow.as_ref(), arrow.as_ref()));
        assert!(!same(None, arrow.as_ref()));
        assert!(!same(arrow.as_ref(), None));
        let two = Some(stack("minecraft:arrow", 2));
        assert!(!same(arrow.as_ref(), two.as_ref()));
        let other = Some(stack("minecraft:stone", 1));
        assert!(!same(arrow.as_ref(), other.as_ref()));
        let mut damaged = stack("minecraft:arrow", 1);
        damaged.damage = 3;
        assert!(!same(arrow.as_ref(), Some(&damaged)));
    }

    /// Every slot the comparison walks is a real `EquipmentSlot` name, and
    /// none is `body` (no mob of this pass holds one there).
    #[test]
    fn the_compared_slots_are_equipment_slots() {
        for slot in SLOTS {
            let name = EQUIPMENT_SLOTS[slot];
            assert!(!name.is_empty());
            assert_ne!(name, "body");
        }
        assert_eq!(EQUIPMENT_SLOTS[finalize::MAINHAND], "mainhand");
        assert_eq!(EQUIPMENT_SLOTS[finalize::HEAD], "head");
        assert_eq!(EQUIPMENT_SLOTS[7], "saddle");
    }
}
