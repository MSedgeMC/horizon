//! `FollowParentGoal.canUse`: a baby looks for a parent on one tick in
//! `reduced(400)`, and the draw comes before the scan.
use super::*;
use crate::mobs::world::with_test_world;
use horizon_core::random::{EntityRandom, LegacyRandom};
use std::sync::Arc;

/// One adult cow ten blocks away, a baby cow at the origin, and one
/// `can_use` per seed with the baby's random set to it: the goal starts
/// exactly when the seed's first `reduced(400)` draw is zero.
#[test]
#[ignore = "daily"]
fn a_baby_looks_for_its_parent_once_in_two_hundred_ticks() {
    let mut mismatches = Vec::new();
    let (mut zeros, mut nonzeros) = (0usize, 0usize);
    if !with_test_world(&[], |w, types| {
        let kind = Arc::new(types.types["minecraft:cow"].clone());
        let mut adult = Mob::new(2, kind.clone(), Vec3::new(10.5, 64.0, 0.5), 0.0);
        adult.on_ground = true;
        let seen = [adult.seen(None, &crate::items::Items::empty())];
        let world = crate::mobs::MobWorld {
            others: &seen,
            ..w.clone()
        };
        let mut m = Mob::new(1, kind, Vec3::new(0.5, 64.0, 0.5), 0.0);
        m.extra.age = -1;
        m.on_ground = true;
        let mut level = crate::health::TestLevel::new(0);
        let mut events = Vec::new();
        let mut cx = Cx {
            w: &world,
            level: &mut level.level(),
            events: &mut events,
        };
        for seed in 0..4096i64 {
            m.random = EntityRandom::own(LegacyRandom::new(seed));
            let mut expected = LegacyRandom::new(seed);
            let gate = expected.next_int_bound(reduced(400)).unwrap_or(1);
            let mut goal = FollowParent::new(1.0);
            let picked = goal.can_use(&mut m, &mut cx);
            if gate == 0 {
                zeros += 1;
            } else {
                nonzeros += 1;
            }
            if picked != (gate == 0) {
                mismatches.push((seed, gate, picked));
            }
        }
    }) {
        return;
    }
    assert!(zeros > 0 && nonzeros > 0, "both draws occur in the seeds");
    assert!(
        mismatches.is_empty(),
        "the goal started without the zero draw (or skipped it): {mismatches:?}"
    );
}
