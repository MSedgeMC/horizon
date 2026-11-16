//! The block a loot roll's `movement_affected_by` predicate reads, taken from
//! the level before a mod-listened roll borrows the game.
use horizon_world::{
    loot::{LootBlockSource, movement_affected_by_pos},
    registry::BlockRegistry,
};

/// The block at the one position `MovementAffectedByPredicate` reads: the
/// centre of `Entity.getBlockPosBelowThatAffectsMyMovement`. The game's roll
/// holds this owned snapshot, so `ListenedRolls` can keep its `&mut Game`.
#[derive(Debug)]
pub struct LootBlockAt {
    pos: [i32; 3],
    block: Option<String>,
}

impl LootBlockAt {
    /// `Level.getBlockState(getBlockPosBelowThatAffectsMyMovement)`, as the
    /// block id (`minecraft:stone`); `None` when that position is unloaded.
    pub fn below(
        position: [f64; 3],
        world: &crate::world::World,
        registry: &BlockRegistry,
    ) -> Self {
        let pos = movement_affected_by_pos(position);
        let block = world
            .chunks
            .get(&(pos[0] >> 4, pos[2] >> 4))
            .and_then(|chunk| {
                let id = chunk.block(pos[0] & 15, pos[1], pos[2] & 15);
                registry.state(id).map(|state| state.name.clone())
            });
        Self { pos, block }
    }
}

impl LootBlockSource for LootBlockAt {
    fn block(&self, pos: [i32; 3]) -> Option<String> {
        if pos == self.pos {
            self.block.clone()
        } else {
            None
        }
    }
}
