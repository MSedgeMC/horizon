//! Custom Item Textures for worn armour and elytra (`type=armor`, `type=elytra`). An armour rule
//! names its textures after the armour texture files of before 1.21.2
//! (`textures/models/armor/diamond_layer_1.png`): `texture.diamond_layer_1`,
//! `texture.leather_layer_2_overlay`..., with `texture` for any layer it does not name. 26.3's
//! equipment assets name the same layers `entity/equipment/humanoid/diamond` (layer 1: helmet,
//! chestplate, boots) and `entity/equipment/humanoid_leggings/diamond` (layer 2), so the layers
//! are mapped back to the old names here. An elytra rule's `texture` replaces the wings'.

use crate::cit_assets::sprite_id;
use horizon_optifine::cit::{Kind, Rule};
use std::collections::BTreeSet;

/// The pre-1.21.2 armour texture name of an equipment asset's layer: `asset` (`minecraft:gold`
/// or `gold`) on the leggings layer (`humanoid_leggings`) or the others (`humanoid`), the dye
/// overlay (`leather_overlay`) or the main texture.
pub fn armor_layer_name(asset: &str, leggings: bool, overlay: bool) -> String {
    let path = asset.rsplit_once(':').map_or(asset, |(_, p)| p);
    // The equipment assets renamed from their old textures; the rest kept their names.
    let old = match path {
        "turtle_scute" => "turtle",
        other => other,
    };
    let layer = if leggings { 2 } else { 1 };
    let suffix = if overlay { "_overlay" } else { "" };
    format!("{old}_layer_{layer}{suffix}")
}

/// The texture (a resource path) `rule` draws on the layer named `layer`: its `texture.<layer>`,
/// else its `texture`.
pub fn armor_texture<'a>(rule: &'a Rule, layer: &str) -> Option<&'a str> {
    rule.textures
        .iter()
        .find(|(name, _)| name == layer)
        .map(|(_, path)| path.as_str())
        .or(rule.texture.as_deref())
}

/// The atlas name of a CIT texture: its `cit_assets` id, without `minecraft:` as the atlas packs
/// vanilla's sprites.
fn atlas_name(resource: &str) -> Option<String> {
    let id = sprite_id(resource)?;
    Some(match id.strip_prefix("minecraft:") {
        Some(name) => name.to_owned(),
        None => id,
    })
}

/// The atlas sprite `rule` puts on the layer of equipment asset `asset` (see
/// [`armor_layer_name`]); `None` keeps the vanilla texture.
pub fn layer_sprite(rule: &Rule, asset: &str, leggings: bool, overlay: bool) -> Option<String> {
    atlas_name(armor_texture(
        rule,
        &armor_layer_name(asset, leggings, overlay),
    )?)
}

/// The atlas sprite an elytra rule puts on the wings.
pub fn elytra_sprite(rule: &Rule) -> Option<String> {
    atlas_name(armor_texture(rule, "elytra")?)
}

/// Sprite ids the armour and elytra rules draw: their `texture` and `texture.<layer>` values. A
/// path no id can name is skipped with a warning.
pub fn armor_sprites(rules: &[Rule]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for rule in rules
        .iter()
        .filter(|r| matches!(r.kind, Kind::Armor | Kind::Elytra))
    {
        let named = rule.textures.iter().map(|(_, path)| path);
        for path in rule.texture.iter().chain(named) {
            match sprite_id(path) {
                Some(id) => {
                    out.insert(id);
                }
                None => log::warn!(
                    "custom item textures: {} names {path}, which is not a valid texture id",
                    rule.path
                ),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_optifine::Properties;

    fn rule(path: &str, text: &str) -> Rule {
        Rule::parse(path, &Properties::parse_equals_first(text), |_| true).unwrap()
    }

    #[test]
    fn layers_take_their_old_texture_names() {
        assert_eq!(
            armor_layer_name("minecraft:diamond", false, false),
            "diamond_layer_1"
        );
        assert_eq!(armor_layer_name("diamond", true, false), "diamond_layer_2");
        assert_eq!(
            armor_layer_name("minecraft:leather", false, true),
            "leather_layer_1_overlay"
        );
        assert_eq!(
            armor_layer_name("minecraft:leather", true, true),
            "leather_layer_2_overlay"
        );
        assert_eq!(
            armor_layer_name("minecraft:gold", false, false),
            "gold_layer_1"
        );
        assert_eq!(
            armor_layer_name("minecraft:chainmail", true, false),
            "chainmail_layer_2"
        );
        assert_eq!(
            armor_layer_name("minecraft:turtle_scute", false, false),
            "turtle_layer_1"
        );
        assert_eq!(
            armor_layer_name("minecraft:netherite", true, false),
            "netherite_layer_2"
        );
    }

    #[test]
    fn a_named_layer_wins_over_the_generic_texture() {
        let r = rule(
            "assets/minecraft/optifine/cit/armor/blue.properties",
            "type=armor\nitems=diamond_chestplate diamond_leggings\ntexture=generic\n\
             texture.diamond_layer_2=blue_legs\n",
        );
        let dir = "assets/minecraft/optifine/cit/armor";
        assert_eq!(
            armor_texture(&r, "diamond_layer_2"),
            Some(format!("{dir}/blue_legs.png").as_str())
        );
        assert_eq!(
            armor_texture(&r, "diamond_layer_1"),
            Some(format!("{dir}/generic.png").as_str())
        );
        assert_eq!(
            layer_sprite(&r, "minecraft:diamond", true, false).as_deref(),
            Some("horizon_cit:minecraft/optifine/cit/armor/blue_legs")
        );
        assert_eq!(
            layer_sprite(&r, "minecraft:diamond", false, false).as_deref(),
            Some("horizon_cit:minecraft/optifine/cit/armor/generic")
        );
    }

    #[test]
    fn without_a_generic_texture_other_layers_stay_vanilla() {
        let r = rule(
            "assets/minecraft/optifine/cit/a.properties",
            "type=armor\nitems=leather_helmet\ntexture.leather_layer_1=dyed\n",
        );
        assert_eq!(
            layer_sprite(&r, "minecraft:leather", false, false).as_deref(),
            Some("horizon_cit:minecraft/optifine/cit/dyed")
        );
        assert_eq!(layer_sprite(&r, "minecraft:leather", false, true), None);
        assert_eq!(layer_sprite(&r, "minecraft:leather", true, false), None);
    }

    #[test]
    fn a_texture_under_textures_keeps_its_vanilla_atlas_name() {
        let r = rule(
            "assets/minecraft/optifine/cit/wings.properties",
            "type=elytra\ntexture=minecraft:textures/entity/wings/dragon.png\n",
        );
        assert_eq!(elytra_sprite(&r).as_deref(), Some("entity/wings/dragon"));
    }

    #[test]
    fn only_armor_and_elytra_rules_reach_the_atlas() {
        let rules = [
            rule(
                "assets/minecraft/optifine/cit/sword.properties",
                "items=diamond_sword\ntexture=sword\n",
            ),
            rule(
                "assets/minecraft/optifine/cit/plate.properties",
                "type=armor\nitems=iron_chestplate\ntexture.iron_layer_1=plate\n",
            ),
            rule(
                "assets/minecraft/optifine/cit/wings.properties",
                "type=elytra\ntexture=wings\n",
            ),
        ];
        let got: Vec<String> = armor_sprites(&rules).into_iter().collect();
        assert_eq!(
            got,
            [
                "horizon_cit:minecraft/optifine/cit/plate",
                "horizon_cit:minecraft/optifine/cit/wings",
            ]
        );
    }
}
