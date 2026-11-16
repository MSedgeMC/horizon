//! What a flying projectile asks of the world (`ProjectileWorld` for `ProjWorld`): block
//! contacts, collisions, fluids and the world border.
use super::*;

pub(crate) struct ProjWorld<'a, 'w> {
    pub(crate) w: &'a MobWorld<'w>,
    pub(crate) targets: &'a [(Who, Aabb)],
    /// The world border's [minX, minZ, maxX, maxZ], which stops the ray as a
    /// block would (`clipIncludingBorder`); None: no border.
    pub(crate) border: Option<[f64; 4]>,
}

impl ProjectileWorld for ProjWorld<'_, '_> {
    fn below_world_y(&self) -> f64 {
        self.w.world.below_world_y()
    }
    fn clip(&self, from: Vec3, to: Vec3) -> Option<(Vec3, [i32; 3], [i32; 3])> {
        let mut scene = SceneCollisions {
            world: self.w.world,
            binding: self.w.collision,
            subject: None,
        };
        let context = horizon_world::collision_context::CollisionContext::position(from.y);
        let r = traverse_blocks(from, to, 256, |p: BlockPos| {
            let state = self.w.world.block(p.x, p.y, p.z);
            if self.w.facts.is_air(state) {
                return None;
            }
            // ClipContext hands CollisionContext.of(projectile): scaffolding
            // is open for a projectile below it.
            let shape = self
                .w
                .collision
                .collision_shape_with_context(&mut scene, state, p, &context)
                .ok()?;
            shape.clip(from, to, p)
        });
        let hit = r.ok().flatten().map(|hit| {
            (
                hit.location,
                [hit.block.x, hit.block.y, hit.block.z],
                hit.direction.normal(),
            )
        });
        match self.border {
            Some(bounds) => clip_including_border(hit, from, to, bounds),
            None => hit,
        }
    }
    /// Entity.updateFluidInteraction over the projectile's box, not the
    /// block at its feet (#3072).
    fn in_water(&self, bounds: Aabb) -> bool {
        self.w.fluids(bounds, bounds.max_position().y).in_water
    }
    fn pushed_by_currents(&self, bounds: Aabb, velocity: Vec3) -> Vec3 {
        self.w.pushed_by_currents(bounds, velocity)
    }
    fn targets(&self) -> &[(Who, Aabb)] {
        self.targets
    }
    /// `Projectile.canHitEntity`'s shooter rule over the mob world's teams
    /// (`Entity.doTeamsAllowDamage`).
    fn can_attack(&self, owner: Option<Who>, who: Who) -> bool {
        // `Entity.canBeHitByProjectile` refuses a spectator.
        if who == Who::Player && self.w.player.as_ref().is_some_and(|p| p.spectator) {
            return false;
        }
        let Some(owner) = owner else {
            return true;
        };
        horizon_client::scoreboard::teams_allow_damage(self.w.teams.of(owner), self.w.teams.of(who))
    }
    /// `BaseFireBlock.entityInside` for a thrown projectile: fire and soul
    /// fire set it alight for 160 ticks.
    fn ignites(&self, p: [i32; 3]) -> bool {
        let state = self.w.world.block(p[0], p[1], p[2]);
        matches!(
            &*self.w.facts.get(state).name,
            "minecraft:fire" | "minecraft:soul_fire"
        )
    }
    fn empty(&self, p: [i32; 3]) -> bool {
        self.w.facts.is_air(self.w.world.block(p[0], p[1], p[2]))
    }
    /// `WebBlock.entityInside` and `PowderSnowBlock.entityInside`.
    fn stuck_speed(&self, p: [i32; 3]) -> Option<Vec3> {
        horizon_client::projectiles::stuck_speed_of(
            &self.w.facts.get(self.w.world.block(p[0], p[1], p[2])).name,
        )
    }
    fn bubble_column(&self, p: [i32; 3]) -> Option<(bool, bool)> {
        horizon_client::motion::bubble_column_at(
            self.w.world,
            self.w.registry,
            self.w.facts,
            self.w.collision,
            BlockPos::new(p[0], p[1], p[2]),
        )
        .map(|c| (c.drag, c.surface))
    }
    fn can_stand_on(&self, p: [i32; 3], entity_y: f64) -> bool {
        horizon_client::collisions::projectile_can_stand_on(
            self.w.world,
            self.w.collision,
            BlockPos::new(p[0], p[1], p[2]),
            entity_y,
        )
    }
}

impl Game<'_> {
    /// `Entity.getTeam` of anyone a projectile can name: the host, a guest
    /// under its entity id, or a mob (its own team, else its tamed owner's).
    fn team_of_who(&self, who: Who) -> Option<&horizon_client::scoreboard::Team> {
        match who {
            Who::Player => self.player_team(),
            Who::Mob(id) => {
                let guest = self
                    .lan
                    .as_ref()
                    .and_then(|h| h.hub.guests.iter().find(|g| g.entity_id as u32 == id));
                match guest {
                    Some(g) => self.score.board.players_team(&g.name),
                    None => self.mob_team(id),
                }
            }
        }
    }
}

impl horizon_client::arrows::ShooterRule for Game<'_> {
    /// `Entity.doTeamsAllowDamage` as `Player.canHarmPlayer` reads it: an
    /// ally of the shooter on a team without friendly fire is not a target.
    fn can_attack(&self, owner: Option<Who>, who: Who) -> bool {
        // `Entity.canBeHitByProjectile` refuses a spectator.
        if who == Who::Player && self.spectator() {
            return false;
        }
        let Some(owner) = owner else {
            return true;
        };
        horizon_client::scoreboard::teams_allow_damage(
            self.team_of_who(owner),
            self.team_of_who(who),
        )
    }
}
