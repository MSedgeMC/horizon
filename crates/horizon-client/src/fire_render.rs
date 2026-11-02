//! Animated burning-entity flames; the first-person overlay uses screen_effects.
use crate::{
    hud::Rect,
    mesher::Vertex,
    projectiles::{Projectile, ProjectileKind},
};
use std::collections::HashMap;

/// `Entity.displayFireAnimation` (`isOnFire() && !isSpectator()`) for the
/// projectiles a frame draws: a burning arrow, or a mob's thrown trident, at
/// the position its previous and current one interpolate to, with the box
/// (`getBbWidth`, `getBbHeight`) `EntityRenderDispatcher` scales the flame by.
pub fn burning_projectiles<'a>(
    projectiles: &'a [Projectile],
    alpha: f64,
) -> impl Iterator<Item = ([f64; 3], f32, f32)> + 'a {
    projectiles.iter().filter_map(move |p| {
        // Only the arrow kinds carry `Entity.remainingFireTicks`, and the
        // flame's size is their entity type's own `sized(...)` box.
        let burning = p.arrow.as_ref().is_some_and(|a| a.on_fire());
        let size = match p.kind {
            ProjectileKind::Arrow { .. } | ProjectileKind::Trident => p.kind.size() as f32,
            _ => return None,
        };
        if !burning {
            return None;
        }
        let (from, to) = (p.previous, p.position);
        Some((
            [
                from.x + (to.x - from.x) * alpha,
                from.y + (to.y - from.y) * alpha,
                from.z + (to.z - from.z) * alpha,
            ],
            size,
            size,
        ))
    })
}

#[allow(clippy::too_many_arguments)]
pub fn flames(
    position: [f64; 3],
    width: f32,
    height: f32,
    camera: glam::DVec3,
    yaw: f32,
    sprites: &HashMap<String, Rect>,
    scene_light: u8,
    out: &mut Vec<Vertex>,
) {
    let (Some(a), Some(b)) = (sprites.get("block/fire_0"), sprites.get("block/fire_1")) else {
        return;
    };
    let scale = width * 1.4;
    if scale <= 0.0 {
        return;
    }
    let origin = glam::DVec3::from(position) - camera;
    let rotation = glam::Mat3::from_rotation_y(std::f32::consts::PI - yaw.to_radians());
    let mut remaining = height / scale;
    let mut half = 0.5f32;
    let mut y = 0.0f32;
    let mut z = 0.3 - (remaining as i32) as f32 * 0.02;
    let mut index = 0;
    while remaining > 0.0 {
        let r = if index % 2 == 0 { a } else { b };
        let (u0, u1) = if (index / 2) % 2 == 0 {
            (r.u0, r.u1)
        } else {
            (r.u1, r.u0)
        };
        for (p, uv) in [
            ([-half, y, z], [u0, r.v1]),
            ([half, y, z], [u1, r.v1]),
            ([half, y + 1.4, z], [u1, r.v0]),
            ([-half, y + 1.4, z], [u0, r.v0]),
        ] {
            let p = origin.as_vec3() + rotation * (glam::Vec3::from(p) * scale);
            out.push(Vertex {
                pos: p.to_array(),
                uv,
                layer: 0,
                color: 0xffff_ffff,
                light: 0xf000 | (u32::from(scene_light >> 4) * 16),
                normal: [0; 4],

                glint_uv: [0.0, 0.0],
            });
        }
        remaining -= 0.45;
        y += 0.45;
        half *= 0.9;
        z -= 0.03;
        index += 1;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn flames_cover_entity_height_and_alternate_animated_sprites() {
        let rect = |u0| super::Rect {
            u0,
            v0: 0.0,
            u1: u0 + 0.1,
            v1: 0.1,
            w: 16,
            h: 16,
        };
        let sprites = super::HashMap::from([
            ("block/fire_0".into(), rect(0.0)),
            ("block/fire_1".into(), rect(0.5)),
        ]);
        let mut out = Vec::new();
        super::flames(
            [0.0; 3],
            0.6,
            1.8,
            glam::DVec3::ZERO,
            0.0,
            &sprites,
            0,
            &mut out,
        );
        assert_eq!(out.len() % 4, 0);
        assert!(
            out[0].pos[2] < 0.0,
            "billboard depth bias must face the camera"
        );
        assert!(
            out.iter().all(|v| v.light == 0xf000),
            "fire forces block light to 15 but preserves scene sky light"
        );
        assert!(out.len() >= 20);
        assert!(out.iter().any(|v| v.pos[1] >= 1.8));
        assert!(
            out.iter().all(|v| v.layer == 0),
            "flames must not use the breaking-cracks UV-remap material"
        );
        assert!(out[..4].iter().all(|v| v.uv[0] <= 0.1));
        assert!(out[4..8].iter().all(|v| v.uv[0] >= 0.5));
    }

    /// A burning arrow was drawn with no flame over it at all: the flame pass
    /// only knew mobs, the player, dropped items and armor stands (#7457). A
    /// Flame arrow is on fire from its first tick (`set_bonus` ignites it for
    /// 100 seconds), and an arrow that flew through a fire block is too.
    #[test]
    fn a_burning_arrow_is_in_the_flame_pass_and_a_plain_one_is_not() {
        use crate::{mobs::Who, mobs::gear::ArrowBonus, projectiles::ProjectileKind};
        let arrow = |position: horizon_core::geometry::Vec3| {
            super::Projectile::new(
                ProjectileKind::Arrow {
                    base_damage: 2.0,
                    critical: false,
                    effect: None,
                },
                position,
                Some(Who::Player),
            )
        };
        let at = |x, y, z| horizon_core::geometry::Vec3::new(x, y, z);
        let mut burning = arrow(at(0.0, 64.0, 0.0));
        assert_eq!(
            super::burning_projectiles(std::slice::from_ref(&burning), 0.5).count(),
            0,
            "an arrow that never touched fire must not carry a flame"
        );
        burning.set_bonus(ArrowBonus {
            flame: true,
            ..ArrowBonus::default()
        });
        burning.position = at(1.0, 66.0, 2.0);
        let listed: Vec<_> =
            super::burning_projectiles(std::slice::from_ref(&burning), 0.5).collect();
        // The render state interpolates the two ticks' positions, and the flame
        // is the entity type's own 0.5 box (`renderFlame` scales by 1.4 of it).
        assert_eq!(listed, vec![([0.5, 65.0, 1.0], 0.5, 0.5)]);
        // A thrown snowball, an egg or a potion has no `remainingFireTicks` at
        // all; a mob's trident flies with the arrow's state and does have them.
        let mut snowball =
            super::Projectile::new(ProjectileKind::Snowball, at(0.0, 64.0, 0.0), None);
        snowball.previous = at(0.0, 64.0, 0.0);
        let mut trident = super::Projectile::new(ProjectileKind::Trident, at(0.0, 64.0, 0.0), None);
        trident.arrow.as_mut().unwrap().ignite_for_ticks(160);
        let sizes: Vec<_> = super::burning_projectiles(&[snowball, trident], 0.0)
            .map(|(_, w, _)| w)
            .collect();
        assert_eq!(
            sizes,
            vec![0.5],
            "only the trident burns, and at its box size"
        );
    }
}
