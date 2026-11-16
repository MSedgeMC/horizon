//! The blocks a loot roll's entity predicates read, taken from the level before
//! a mod-listened roll borrows the game.
use horizon_world::{
    loot::{LootBlockSource, movement_affected_by_pos},
    registry::BlockRegistry,
};

/// The blocks `MovementAffectedByPredicate` and `EntityLocationPredicate` read:
/// the centre of `Entity.getBlockPosBelowThatAffectsMyMovement` and, for a
/// `location` predicate, the block at `Entity.getX/getY/getZ`. The game's roll
/// holds this owned snapshot, so `ListenedRolls` can keep its `&mut Game`.
#[derive(Debug)]
pub struct LootBlockAt {
    below: ([i32; 3], Option<String>),
    at: Option<([i32; 3], Option<String>)>,
}

impl LootBlockAt {
    /// `Level.getBlockState(pos)`, as the block id (`minecraft:stone`); `None`
    /// when that position is unloaded.
    fn block_at(
        pos: [i32; 3],
        world: &crate::world::World,
        registry: &BlockRegistry,
    ) -> Option<String> {
        world
            .chunks
            .get(&(pos[0] >> 4, pos[2] >> 4))
            .and_then(|chunk| {
                let id = chunk.block(pos[0] & 15, pos[1], pos[2] & 15);
                registry.state(id).map(|state| state.name.clone())
            })
    }

    /// `Level.getBlockState(getBlockPosBelowThatAffectsMyMovement)` only: the
    /// block `movement_affected_by` reads.
    pub fn below(
        position: [f64; 3],
        world: &crate::world::World,
        registry: &BlockRegistry,
    ) -> Self {
        let pos = movement_affected_by_pos(position);
        Self {
            below: (pos, Self::block_at(pos, world, registry)),
            at: None,
        }
    }

    /// The block below the entity and the block at its own position, the
    /// latter for `location` (`LocationPredicate.block` at `Entity.position`).
    pub fn at_entity(
        position: [f64; 3],
        world: &crate::world::World,
        registry: &BlockRegistry,
    ) -> Self {
        let at = [
            position[0].floor() as i32,
            position[1].floor() as i32,
            position[2].floor() as i32,
        ];
        let mut blocks = Self::below(position, world, registry);
        if at != blocks.below.0 {
            blocks.at = Some((at, Self::block_at(at, world, registry)));
        }
        blocks
    }
}

impl LootBlockSource for LootBlockAt {
    fn block(&self, pos: [i32; 3]) -> Option<String> {
        if pos == self.below.0 {
            return self.below.1.clone();
        }
        self.at
            .as_ref()
            .filter(|(at, _)| *at == pos)
            .and_then(|(_, block)| block.clone())
    }
}
