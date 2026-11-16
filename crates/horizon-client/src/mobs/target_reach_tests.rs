//! `TargetGoal.canReach`: a must-reach target goal (the polar bear's player
//! and fox goals, a llama's wolf goal, an enderman's endermite goal) only
//! takes a target it can path to, or reach with a melee attack in sight.
use super::*;
use crate::mobs::goals::{Cx, NearestTarget};
use crate::mobs::world::with_built_test_world;
use horizon_core::geometry::Vec3;
use std::sync::Arc;

/// Whether the fox target is picked, once with `reach()` and once without,
/// on a shelf with the fox either on the floor or on a three block pillar.
fn picked(on_pillar: bool) -> Option<(bool, bool)> {
    let mut kinds = None;
    with_built_test_world(&[], shelf(false), |_, types| {
        kinds = Some((
            Arc::new(types.types["minecraft:polar_bear"].clone()),
            Arc::new(types.types["minecraft:fox"].clone()),
        ));
    });
    let (bear_kind, fox_kind) = kinds?;
    let mut fox = Mob::new(
        2,
        fox_kind,
        Vec3::new(6.5, if on_pillar { 67.0 } else { 64.0 }, 0.5),
        0.0,
    );
    fox.on_ground = true;
    let seen = [fox.seen(None, &crate::items::Items::empty())];
    let mut out = None;
    with_built_test_world(&seen, shelf(on_pillar), |w, _| {
        let pick = |reach: bool| {
            let mut m = Mob::new(1, bear_kind.clone(), Vec3::new(0.5, 64.0, 0.5), 0.0);
            m.on_ground = true;
            let mut goal = NearestTarget::species(&[Species::Fox], true).interval(1);
            if reach {
                goal = goal.reach();
            }
            let mut level = crate::health::TestLevel::new(0);
            let mut events = Vec::new();
            let mut cx = Cx {
                w,
                level: &mut level.level(),
                events: &mut events,
            };
            (0..3).any(|_| goal.can_use(&mut m, &mut cx))
        };
        out = Some((pick(false), pick(true)));
    });
    out
}

/// A stone shelf at y 63; on it, two blocks of stone stack up at (6, 0).
fn shelf(
    pillar: bool,
) -> impl FnOnce(&mut crate::world::World, &horizon_world::registry::BlockRegistry) {
    move |world, registry| {
        for cx in -1..=1 {
            for cz in -1..=1 {
                world
                    .chunks
                    .insert((cx, cz), Arc::new(crate::chunk::Chunk::empty(cx, cz)));
            }
        }
        let stone = registry.default_state("minecraft:stone").unwrap();
        for x in -8..=8 {
            for z in -8..=8 {
                world.set(x, 63, z, stone).unwrap();
            }
        }
        if pillar {
            for y in 64..=66 {
                world.set(6, y, 0, stone).unwrap();
            }
        }
    }
}

/// `TargetGoal.canReach` (`MeleeAttackGoal.canUse`'s fallback included): a
/// fox on the shelf is taken, a fox atop a three block pillar is not. The
/// plain goal takes the pillar fox, so only `reach()` refuses it.
#[test]
#[ignore = "daily"]
fn a_must_reach_target_goal_skips_a_target_it_cannot_path_to() {
    let Some((plain, reach)) = picked(true) else {
        return;
    };
    assert!(plain, "the pillar fox is in range and in sight");
    assert!(!reach, "the goal took a fox it cannot path to");
    let Some((plain, reach)) = picked(false) else {
        return;
    };
    assert!(plain, "the shelf fox is in range and in sight");
    assert!(reach, "the goal keeps a fox it can path to");
}
