//! `LivingEntity.onEquipItem` for players: the equip sound and the equip
//! game event when an armour or off-hand slot takes a different item, from
//! every path that calls it (use, the inventory menu, the creative screen,
//! the swap-hands key, dispensers), for the host and for LAN guests.
use super::{Game, equipment_modifier_applies};
use horizon_client::items::ItemStack;

/// The equipment slots `onEquipItem` can act on, as `EquipmentSlot` names:
/// the armour (`Player.doesEmitEquipEvent`: these emit the game event) and
/// the off hand.
const ARMOR: [&str; 4] = ["head", "chest", "legs", "feet"];
const OFFHAND: &str = "offhand";

/// `ItemStack.isSameItemSameComponents` over optional stacks.
fn same(a: Option<&ItemStack>, b: Option<&ItemStack>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same_item_same_components(b),
        _ => false,
    }
}

/// `ItemStack.get(EQUIPPABLE)`: the stack's slot and equip sound, else the
/// item's own. A present component without `equip_sound` is the generic
/// armour sound (None).
fn stack_equippable(
    stack: &ItemStack,
    default_slot: Option<&str>,
    default_sound: Option<String>,
) -> Option<(String, Option<String>)> {
    let data = stack.data();
    if data.is_some_and(|d| d.other("!minecraft:equippable").is_some()) {
        return None;
    }
    if let Some(equippable) = data.and_then(|d| d.other("minecraft:equippable")) {
        let slot = match equippable.get("slot") {
            Some(horizon_world::nbt::Tag::String(slot)) => slot.to_string_lossy(),
            _ => return None,
        };
        let sound = match equippable.get("equip_sound") {
            Some(horizon_world::nbt::Tag::String(sound)) => Some(sound.to_string_lossy()),
            _ => None,
        };
        return Some((slot, sound));
    }
    Some((default_slot?.to_owned(), default_sound))
}

impl Game<'_> {
    /// The armour (head first) and the off hand, in `EquipmentSlot` order of
    /// the inventory menu's slots.
    pub(super) fn player_equipment(&self) -> [Option<ItemStack>; 5] {
        let [head, chest, legs, feet] = self.armor.clone();
        [head, chest, legs, feet, self.offhand.clone()]
    }

    /// `onEquipItem` for every slot that changed since `before`: what the
    /// server does for the creative screen, which sends each changed slot
    /// (`handleSetCreativeModeSlot` sets it by player).
    pub(super) fn player_equipment_changed(&mut self, before: &[Option<ItemStack>; 5]) {
        let after = self.player_equipment();
        for (i, slot) in ARMOR.into_iter().chain([OFFHAND]).enumerate() {
            self.player_on_equip(slot, before[i].as_ref(), after[i].as_ref());
        }
    }

    /// `LivingEntity.onEquipItem(slot, old, new)` for the player in place
    /// (the host, or a guest inside `as_guest`).
    pub(super) fn player_on_equip(
        &mut self,
        slot: &str,
        old: Option<&ItemStack>,
        new: Option<&ItemStack>,
    ) {
        if !same(old, new) {
            self.player_equipped(slot, new);
        }
    }

    /// The body of `onEquipItem` once the item changed: the item's
    /// `Equippable.equipSound` when it is equipped in its own slot, and the
    /// `equip` / `unequip` game event for armour. Nothing on a client (the
    /// server plays it) or for a spectator. The player's `firstTick` never
    /// sees an equip here: loading a save sets the slots directly.
    pub(super) fn player_equipped(&mut self, slot: &str, new: Option<&ItemStack>) {
        if self.remote.is_some() || self.spectator() {
            return;
        }
        self.native_on_equip(slot, new);
        let Some(at) = self.player.as_ref().map(|p| p.position) else {
            return;
        };
        let at = [at.x, at.y, at.z];
        let equippable = new.and_then(|stack| {
            let info = self.items().get(&stack.item)?;
            let (own, sound) =
                stack_equippable(stack, info.equip_slot.as_deref(), info.equip_sound.clone())?;
            Some((own == slot, sound))
        });
        if let Some((true, sound)) = &equippable {
            let sound = sound
                .as_deref()
                .unwrap_or("minecraft:item.armor.equip_generic");
            // Player.getSoundSource: PLAYERS.
            let players = horizon_client::settings::SOUND_SOURCES
                .iter()
                .position(|s| *s == "player")
                .unwrap_or(0);
            self.audio.play_on(players, sound, Some(at), 1.0, 1.0);
        }
        if ARMOR.contains(&slot) {
            let event = if equippable.is_some() {
                "minecraft:equip"
            } else {
                "minecraft:unequip"
            };
            let ctx = self.player_context(None);
            self.post_game_event(event, at, ctx);
        }
    }

    /// Equipment attribute modifiers (LivingEntity.collectEquipmentChanges).
    pub(super) fn apply_equipment(&mut self) {
        let Some(player) = self.player.as_mut() else {
            return;
        };
        for (attribute, id) in std::mem::take(&mut self.applied_modifiers) {
            let _ = player.set_modifier_unclamped(&attribute, &id, 0.0, "add_value", false);
        }
        let slots = ["head", "chest", "legs", "feet"];
        let mut equipped: Vec<(Option<&ItemStack>, &str)> = self
            .armor
            .iter()
            .zip(slots)
            .map(|(s, n)| (s.as_ref(), n))
            .collect();
        equipped.push((self.inventory[self.selected].as_ref(), "mainhand"));
        equipped.push((self.offhand.as_ref(), "offhand"));
        for (stack, slot) in equipped {
            let Some(stack) = stack else {
                continue;
            };
            let modifiers = world_items!(self).attribute_modifiers(stack);
            for (attribute, id, amount, op, group) in &modifiers {
                if !equipment_modifier_applies(group, slot) {
                    continue;
                }
                if player
                    .set_modifier_unclamped(attribute, id, *amount, op, true)
                    .is_ok()
                {
                    self.applied_modifiers.push((attribute.clone(), id.clone()));
                }
            }
        }
        let _ = player.clamp_health_to_max();
        self.apply_enchantment_attributes();
    }
}

#[cfg(test)]
mod equippable_tests {
    use super::stack_equippable;
    use horizon_client::items::ItemStack;

    #[test]
    fn equip_sound_comes_from_the_stack() {
        let mut helmet = ItemStack::new("minecraft:golden_helmet", 1);
        assert_eq!(
            stack_equippable(&helmet, Some("head"), Some("gold".into())),
            Some(("head".into(), Some("gold".into())))
        );
        helmet.data_mut().set_other(
            "minecraft:equippable",
            Some(horizon_world::nbt::Tag::compound([(
                "slot",
                horizon_world::nbt::Tag::String("head".into()),
            )])),
        );
        assert_eq!(
            stack_equippable(&helmet, Some("head"), Some("gold".into())),
            Some(("head".into(), None))
        );
        helmet.data_mut().set_other(
            "!minecraft:equippable",
            Some(horizon_world::nbt::Tag::compound(Vec::<(
                &str,
                horizon_world::nbt::Tag,
            )>::new())),
        );
        assert_eq!(
            stack_equippable(&helmet, Some("head"), Some("gold".into())),
            None
        );
    }
}
