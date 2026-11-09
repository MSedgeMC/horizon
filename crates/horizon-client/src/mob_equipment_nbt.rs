//! `LivingEntity`'s saved `equipment` (`EntityEquipment.CODEC`).
//!
//! The codec is an unbounded map from `EquipmentSlot.CODEC` to
//! `ItemStack.CODEC`: an unknown slot name or an item that fails its codec
//! fails the whole map, and `readAdditionalSaveData` then sets an empty
//! `EntityEquipment` (`Optional.orElseGet`). Horizon read each slot on its
//! own, so one bad slot left the rest of the equipment in place.
use crate::mobs::Mob;
use horizon_world::nbt::{Compound, Tag};

/// `EquipmentSlot` names (`mainhand` ... `saddle`).
const SLOTS: [&str; 8] = [
    "mainhand", "offhand", "feet", "legs", "chest", "head", "body", "saddle",
];

/// `LivingEntity.readAdditionalSaveData`'s `equipment`: what the mob holds,
/// when the whole map decodes; nothing otherwise.
pub fn read_saved(m: &mut Mob, tag: &Tag) {
    let Some(value) = tag.get("equipment") else {
        // Registration hands out default weapons (finalizeSpawn does in
        // vanilla); a loaded mob holds only what its file lists.
        m.extra.set_main_hand(None);
        return;
    };
    let Tag::Compound(eq) = value else {
        clear(m);
        return;
    };
    if eq.keys().any(|key| !SLOTS.contains(&key.to_string_lossy().as_str()))
        || eq
            .values()
            .any(|stack| crate::save::item_from_nbt(stack).is_none())
    {
        clear(m);
        return;
    }
    apply(m, eq);
}

fn clear(m: &mut Mob) {
    m.extra.set_main_hand(None);
    m.extra.off_hand = None;
    m.extra.head = None;
    m.extra.chest = None;
    m.extra.legs = None;
    m.extra.feet = None;
    m.extra.body = None;
    for index in 0..crate::mobs::gear::KEYS.len() {
        crate::mobs::gear::set(m, index, Vec::new());
    }
}

fn apply(m: &mut Mob, eq: &Compound) {
    m.extra
        .set_main_hand_stack(eq.get(&"mainhand".into()).and_then(crate::save::item_from_nbt));
    m.extra.off_hand = eq
        .get(&"offhand".into())
        .and_then(|i| crate::save::text(i.get("id")))
        .map(Into::into);
    if !crate::mobs::equine::is_equine(m.species()) {
        let slot = |key: &str| {
            eq.get(&key.into())
                .and_then(|i| crate::save::text(i.get("id")))
                .map(Into::into)
        };
        m.extra.head = slot("head");
        // A happy ghast wears its harness in the body slot (and a nautilus
        // its armour, a sulfur cube its swallowed block).
        if crate::mobs::species::body_slot(m.species()) {
            m.extra.body = slot("body");
        }
        m.extra.chest = slot("chest");
        m.extra.legs = slot("legs");
        m.extra.feet = slot("feet");
    }
    // Their enchantments and trims.
    for (index, key) in crate::mobs::gear::KEYS.iter().enumerate() {
        if let Some(stack) = eq.get(&(*key).into()).and_then(crate::save::item_from_nbt) {
            crate::mobs::gear::set(m, index, stack.enchantments().to_vec());
            let trim = stack.data().and_then(|d| d.trim.clone());
            crate::mobs::gear::set_trim(m, index, trim);
            // The damage, name and components stay with the slot.
            if index != crate::mobs::gear::MAIN {
                crate::mobs::gear::set_stack(m, index, stack);
            }
        }
    }
}
