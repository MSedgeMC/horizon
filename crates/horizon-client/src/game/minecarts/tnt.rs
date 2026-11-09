//! `MinecartTNT`'s saved blast settings: the base power and the speed
//! factor, both clamped to `0..=128` on load.
use super::Minecart;
use crate::game::Game;
use horizon_core::random::RandomSource;
use horizon_world::nbt::Tag;

/// `MinecartTNT.addAdditionalSaveData`: each field only off its default.
pub(super) fn write(c: &Minecart, put: &mut dyn FnMut(&str, Tag)) {
    if c.explosion_power != 4.0 {
        put("explosion_power", Tag::Float(c.explosion_power.to_bits()));
    }
    if c.explosion_speed_factor != 1.0 {
        put(
            "explosion_speed_factor",
            Tag::Float(c.explosion_speed_factor.to_bits()),
        );
    }
}

/// `MinecartTNT.readAdditionalSaveData`: `getFloatOr` then
/// `Mth.clamp(0, 128)` for each field.
pub(super) fn read(c: &mut Minecart, tag: &Tag) {
    let real = |key, default| {
        horizon_client::save::float(tag.get(key))
            .map_or(default, |v| v as f32)
            .clamp(0.0, 128.0)
    };
    c.explosion_power = real("explosion_power", 4.0);
    c.explosion_speed_factor = real("explosion_speed_factor", 1.0);
}

/// `MinecartTNT.explode`: the base power plus the saved speed factor times
/// `random.nextDouble() * 1.5 * min(sqrt(speedSqr), 5)`.
pub(super) fn explode_cart(game: &mut Game<'_>, c: &mut Minecart, speed_sqr: f64) {
    if !game.ui.rules.bool("tnt_explodes") {
        return;
    }
    let speed = speed_sqr.sqrt().min(5.0);
    let r = c.random.next_double();
    let power = (f64::from(c.explosion_power)
        + f64::from(c.explosion_speed_factor) * r * 1.5 * speed) as f32;
    let at = [c.position.x, c.position.y, c.position.z];
    // The cart is the explosion's source: it is gone before the blast
    // looks for entities, so it can't hit itself.
    let silent = game.entity_silent(c.id);
    if let Some(i) = game.carts.iter().position(|x| x.id == c.id) {
        game.eject_cart_mobs(i);
    }
    game.release_vehicle_container(c.id);
    game.carts.retain(|x| x.id != c.id);
    // ExplosionInteraction.TNT: the tnt_explosion_drop_decay rule.
    let interaction = game.destroy_type("tnt_explosion_drop_decay");
    // MinecartTNT.isPrimed (fuse > -1): a cart that ran into a wall or
    // fell unprimed blows rails up like any blast.
    game.combat.rail_proof_blast = c.fuse >= 0;
    game.combat.explosion_in_water = game.bounds_in_water(c.bounds());
    let _ = game.with_hive_emptying_blast(|g| {
        g.server_explode_from(
            at,
            power,
            interaction,
            false,
            crate::game::explosion::ExplosionSource::default()
                .with_silence(silent)
                .with_entity(c.id, "minecraft:tnt_minecart"),
        )
    });
    game.combat.explosion_in_water = false;
    game.combat.rail_proof_blast = false;
}
