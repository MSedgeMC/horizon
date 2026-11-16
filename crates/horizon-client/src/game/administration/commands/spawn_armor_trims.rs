//! `SpawnArmorTrimsCommand`: `/spawn_armor_trims (*_lag_my_game|<pattern>)`,
//! the development command that lays out every trim pattern and material
//! around the player, one armor stand per equippable armor item.
use super::{CommandError, Game, R, Text, err};
use crate::{item_data::trim::Holder, items::ItemStack};
use horizon_client::command::{Arg, Context};
use horizon_core::nbt::Tag;
use horizon_items::equipment::{Equipment, SLOT_NAMES};

/// `SpawnArmorTrimsCommand.VANILLA_TRIM_PATTERNS`, the order
/// `Util.createIndexLookup` sorts the registry to.
const VANILLA_TRIM_PATTERNS: [&str; 18] = [
    "minecraft:sentry",
    "minecraft:dune",
    "minecraft:coast",
    "minecraft:wild",
    "minecraft:ward",
    "minecraft:eye",
    "minecraft:vex",
    "minecraft:tide",
    "minecraft:snout",
    "minecraft:rib",
    "minecraft:spire",
    "minecraft:wayfinder",
    "minecraft:shaper",
    "minecraft:silence",
    "minecraft:raiser",
    "minecraft:host",
    "minecraft:flow",
    "minecraft:bolt",
];
/// `SpawnArmorTrimsCommand.VANILLA_TRIM_MATERIALS`, in the same lookup order.
const VANILLA_TRIM_MATERIALS: [&str; 11] = [
    "minecraft:quartz",
    "minecraft:iron",
    "minecraft:netherite",
    "minecraft:redstone",
    "minecraft:copper",
    "minecraft:gold",
    "minecraft:emerald",
    "minecraft:diamond",
    "minecraft:lapis",
    "minecraft:amethyst",
    "minecraft:resin",
];

/// `Util.createIndexLookup`: an element's index in `order`, or -1 when it is
/// not listed. Java's `Stream.sorted` is stable and so is `sort_by_key`, so
/// unlisted (modded) ids stay in registry order, ahead of the vanilla ones.
fn vanilla_order(ids: &[String], order: &[&str]) -> Vec<String> {
    let mut ids = ids.to_vec();
    ids.sort_by_key(|id| {
        order
            .iter()
            .position(|ordered| ordered == id)
            .map_or(-1, |i| i as i32)
    });
    ids
}

/// `EquipmentSlot.Type.HUMANOID_ARMOR`, the slots
/// `SpawnArmorTrimsCommand.findEquippableItemsWithAssets` keeps.
fn is_humanoid_armor_slot(slot: &str) -> bool {
    matches!(slot, "head" | "chest" | "legs" | "feet")
}

/// `ArmorStand.setItemSlot(equippable.slot(), stack)` after
/// `stack.set(DataComponents.TRIM, trim)`; the `equipment` compound a saved
/// armor stand reads back.
fn trimmed_stand_equipment(slot: &str, item: &str, pattern: &str, material: &str) -> Option<Tag> {
    if !is_humanoid_armor_slot(slot) {
        return None;
    }
    let index = SLOT_NAMES.iter().position(|name| *name == slot)?;
    let mut stack = ItemStack::new(item, 1);
    stack.data_mut().trim = Some((
        Holder::Named(material.into()),
        Holder::Named(pattern.into()),
    ));
    let mut equipment = Equipment::default();
    if !equipment.set(index, Some(stack)) {
        return None;
    }
    equipment.write()
}

/// One spawned stand: `new ArmorStand(level, x, y, z)`, `setYRot(180.0F)`,
/// `setNoGravity(true)`, its equipment, and either the trim's name (the first
/// item of a row) or `setInvisible(true)` (the rest).
fn stand_tag(pos: [f64; 3], equipment: Tag, name: Option<Text>) -> Tag {
    let mut tag = Tag::compound([
        ("id", Tag::String("minecraft:armor_stand".into())),
        (
            "Pos",
            Tag::List(pos.map(|v| Tag::Double(v.to_bits())).to_vec()),
        ),
        (
            "Rotation",
            Tag::List(vec![
                Tag::Float(180.0f32.to_bits()),
                Tag::Float(0.0f32.to_bits()),
            ]),
        ),
        ("NoGravity", Tag::Byte(1)),
        ("equipment", equipment),
    ]);
    if let Tag::Compound(map) = &mut tag {
        match name {
            Some(name) => {
                map.insert(
                    "CustomName".into(),
                    crate::scoreboard::json_to_tag(&name.component()),
                );
                map.insert("CustomNameVisible".into(), Tag::Byte(1));
            }
            None => {
                map.insert("Invisible".into(), Tag::Byte(1));
            }
        }
    }
    tag
}

/// `TrimPattern.copyWithStyle(material).copy().append(" & ").append(material.description())`:
/// the data packs' descriptions are `trim_pattern.minecraft.<path>` and
/// `trim_material.minecraft.<path>` translation components.
fn trim_name(pattern: &str, material: &str) -> Text {
    // Vanilla's own descriptions are `trim_pattern.minecraft.<path>` and
    // `trim_material.minecraft.<path>`; a data pack names its namespace.
    let key = |kind: &str, id: &str| match id.split_once(':') {
        Some((namespace, path)) => format!("trim_{kind}.{namespace}.{path}"),
        None => format!("trim_{kind}.minecraft.{id}"),
    };
    Text::translate(&key("pattern", pattern), &[])
        .append(" & ")
        .append(Text::translate(&key("material", material), &[]))
}

impl Game<'_> {
    pub(in crate::game) fn cmd_spawn_armor_trims(&mut self, id: &str, ctx: &Context) -> R<i32> {
        // Both forms call `getPlayerOrException()` first.
        self.source_player()?;
        let Some((player, yaw)) = self.player.as_ref().map(|p| (p.position, p.yaw)) else {
            return Err(err("permissions.requires.player", &[]));
        };
        let patterns = self.ui.registries.ids("minecraft:trim_pattern").to_vec();
        let patterns = if id == "spawn_armor_trims.all" {
            vanilla_order(&patterns, &VANILLA_TRIM_PATTERNS)
        } else {
            let Some(Arg::Resource(pattern)) = ctx.get("pattern").cloned() else {
                return Ok(0);
            };
            if !patterns.iter().any(|known| known == &pattern) {
                // `ERROR_INVALID_PATTERN`, from `ResourceKeyArgument.getRegistryKey`.
                return Err(CommandError::new(Text::translate(
                    "Invalid pattern",
                    &[Text::literal(&pattern)],
                )));
            }
            vec![pattern]
        };
        let materials = vanilla_order(
            self.ui.registries.ids("minecraft:trim_material"),
            &VANILLA_TRIM_MATERIALS,
        );
        // `findEquippableItemsWithAssets`, in item registry order.
        let equippable: Vec<(String, String)> = {
            let items = self.res.items();
            items
                .registry_order
                .iter()
                .filter_map(|item| {
                    let info = items.items.get(item)?;
                    let slot = info.equip_slot.as_deref()?;
                    (is_humanoid_armor_slot(slot) && info.equip_asset.is_some())
                        .then(|| ((**item).to_owned(), slot.to_owned()))
                })
                .collect()
        };
        // `player.blockPosition().relative(player.getDirection(), 5)`
        // (`Direction.fromYRot`: south, west, north, east).
        let at = [
            player.x.floor() as i32,
            player.y.floor() as i32,
            player.z.floor() as i32,
        ];
        let facing = ((f64::from(yaw) / 90.0 + 0.5).floor() as i32) & 3;
        let (dx, dz) = [(0, 1), (-1, 0), (0, -1), (1, 0)][facing as usize];
        let origin = [at[0] + dx * 5, at[1], at[2] + dz * 5];

        for (material_index, material) in materials.iter().enumerate() {
            for (pattern_index, pattern) in patterns.iter().enumerate() {
                for (item_index, (item, slot)) in equippable.iter().enumerate() {
                    let x = f64::from(origin[0]) + 0.5 - item_index as f64 * 3.0;
                    let y = f64::from(origin[1]) + 0.5 + material_index as f64 * 3.0;
                    let z = f64::from(origin[2]) + 0.5 + pattern_index as f64 * 10.0;
                    let Some(equipment) = trimmed_stand_equipment(slot, item, pattern, material)
                    else {
                        continue;
                    };
                    let name = (item_index == 0).then(|| trim_name(pattern, material));
                    let _ = self.spawn_entity_from_nbt(stand_tag([x, y, z], equipment, name));
                }
            }
        }
        self.feedback(Text::literal(
            "Armorstands with trimmed armor spawned around you",
        ));
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entity_nbt::armor_stand_from_nbt, save::item_from_nbt};
    use horizon_client::command::{Dispatcher, Node, NodeKind, Registries, vanilla};

    fn root_child<'a>(d: &'a Dispatcher, name: &str) -> &'a Node {
        d.nodes[0]
            .children
            .iter()
            .map(|&child| &d.nodes[child])
            .find(|node| node.name() == name)
            .unwrap_or_else(|| panic!("no /{name}"))
    }

    /// `spawn_armor_trims` is registered only in the development tree, under
    /// GAMEMASTERS, with both vanilla forms; the stack the command builds
    /// carries the trim and the stand reads it back.
    #[test]
    fn spawn_armor_trims_is_a_dev_command_and_builds_trimmed_stands() {
        let d = vanilla::build_with(true);
        let spawn = root_child(&d, "spawn_armor_trims");
        assert_eq!(spawn.permission, 2, "Commands.LEVEL_GAMEMASTERS");
        let pattern = spawn
            .children
            .iter()
            .map(|&child| &d.nodes[child])
            .find(|node| node.name() == "pattern")
            .expect("pattern argument");
        assert!(matches!(
            &pattern.kind,
            NodeKind::Argument {
                ty: horizon_client::command::ArgType::ResKey(registry),
                ..
            } if *registry == "minecraft:trim_pattern"
        ));
        let all = spawn
            .children
            .iter()
            .map(|&child| &d.nodes[child])
            .find(|node| node.name() == "*_lag_my_game")
            .expect("the all form");
        assert_eq!(all.command, Some("spawn_armor_trims.all"));

        let mut registries = Registries::default();
        registries.permission = 2;
        let parsed = d.parse("spawn_armor_trims coast", &registries);
        assert_eq!(
            parsed.executable().expect("pattern form parses").1,
            "spawn_armor_trims"
        );
        let parsed = d.parse("spawn_armor_trims *_lag_my_game", &registries);
        assert_eq!(
            parsed.executable().expect("all form parses").1,
            "spawn_armor_trims.all"
        );

        let release = vanilla::build_for(false, false);
        assert!(
            release.nodes[0]
                .children
                .iter()
                .all(|&child| release.nodes[child].name() != "spawn_armor_trims"),
            "release trees leave the development command out"
        );

        let equipment = trimmed_stand_equipment(
            "chest",
            "minecraft:diamond_chestplate",
            "minecraft:coast",
            "minecraft:gold",
        )
        .expect("a chest slot");
        assert!(
            trimmed_stand_equipment(
                "body",
                "minecraft:wolf_armor",
                "minecraft:coast",
                "minecraft:gold"
            )
            .is_none(),
            "only humanoid armor slots"
        );
        let stack = item_from_nbt(equipment.get("chest").expect("chest")).expect("an item stack");
        let (material, pattern) = crate::item_data::trim::named(
            stack
                .data()
                .and_then(|data| data.trim.as_ref())
                .expect("trim"),
        )
        .expect("named holders");
        assert_eq!((material, pattern), ("minecraft:gold", "minecraft:coast"));

        let named = stand_tag(
            [1.5, 64.0, -2.5],
            equipment.clone(),
            Some(trim_name("minecraft:coast", "minecraft:gold")),
        );
        let stand = armor_stand_from_nbt(&named, 1).expect("a stand");
        assert_eq!(stand.position.x, 1.5);
        assert_eq!(stand.yaw, 180.0);
        assert!(stand.no_gravity);
        assert!(stand.name_visible && !stand.invisible);
        let worn = stand.equipment[horizon_items::equipment::CHEST]
            .as_ref()
            .expect("the chest piece");
        assert!(worn.data().and_then(|data| data.trim.as_ref()).is_some());
        let name = stand.custom_name.as_ref().expect("the trim name");
        let json = name.component().to_string();
        assert!(json.contains("trim_pattern.minecraft.coast"), "{json}");
        assert!(json.contains("trim_material.minecraft.gold"), "{json}");

        let hidden = armor_stand_from_nbt(&stand_tag([0.0, 0.0, 0.0], equipment, None), 2)
            .expect("a hidden stand");
        assert!(hidden.invisible && !hidden.name_visible);
        assert!(hidden.custom_name.is_none());
    }
}
