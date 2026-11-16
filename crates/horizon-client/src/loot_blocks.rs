//! The blocks and biomes a loot roll's entity predicates read, taken from the
//! level before a mod-listened roll borrows the game.
use horizon_world::{
    loot::{LootBlockSource, movement_affected_by_pos},
    registry::BlockRegistry,
};

/// What `MovementAffectedByPredicate` and `EntityLocationPredicate` read: the
/// centre of `Entity.getBlockPosBelowThatAffectsMyMovement` and, for a
/// `location` predicate, the block and biome at `Entity.getX/getY/getZ`. The
/// game's roll holds this owned snapshot, so `ListenedRolls` can keep its
/// `&mut Game`.
#[derive(Debug)]
pub struct LootBlockAt {
    below: Snapshot,
    at: Option<Snapshot>,
}

/// One position's `Level.getBlockState` and `Level.getBiome` answers.
#[derive(Debug)]
struct Snapshot {
    pos: [i32; 3],
    block: Option<String>,
    /// `LocationPredicate.biomes` reads the biome at the queried position.
    biome: Option<String>,
}

impl Snapshot {
    fn new(
        pos: [i32; 3],
        world: &crate::world::World,
        registry: &BlockRegistry,
        biomes: &crate::biome::Biomes,
    ) -> Self {
        Self {
            pos,
            block: Self::block_at(pos, world, registry),
            biome: Self::biome_at(pos, world, biomes),
        }
    }

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

    /// `Level.getBiome(pos)`, as the biome id (`minecraft:jungle`); `None` when
    /// that position is unloaded.
    fn biome_at(
        pos: [i32; 3],
        world: &crate::world::World,
        biomes: &crate::biome::Biomes,
    ) -> Option<String> {
        let chunk = world.chunks.get(&(pos[0] >> 4, pos[2] >> 4))?;
        Some(biomes.get(chunk.biome(pos[0], pos[1], pos[2])).name.clone())
    }
}

impl LootBlockAt {
    /// `Level.getBlockState(getBlockPosBelowThatAffectsMyMovement)` and its
    /// biome: what `movement_affected_by` reads.
    pub fn below(
        position: [f64; 3],
        world: &crate::world::World,
        registry: &BlockRegistry,
        biomes: &crate::biome::Biomes,
    ) -> Self {
        let pos = movement_affected_by_pos(position);
        Self {
            below: Snapshot::new(pos, world, registry, biomes),
            at: None,
        }
    }

    /// The block and biome below the entity and at its own position, the
    /// latter for `location` (`LocationPredicate` at `Entity.position`).
    pub fn at_entity(
        position: [f64; 3],
        world: &crate::world::World,
        registry: &BlockRegistry,
        biomes: &crate::biome::Biomes,
    ) -> Self {
        let at = [
            position[0].floor() as i32,
            position[1].floor() as i32,
            position[2].floor() as i32,
        ];
        let mut blocks = Self::below(position, world, registry, biomes);
        if at != blocks.below.pos {
            blocks.at = Some(Snapshot::new(at, world, registry, biomes));
        }
        blocks
    }
}

impl LootBlockSource for LootBlockAt {
    fn block(&self, pos: [i32; 3]) -> Option<String> {
        if pos == self.below.pos {
            return self.below.block.clone();
        }
        self.at
            .as_ref()
            .filter(|at| at.pos == pos)
            .and_then(|at| at.block.clone())
    }

    fn biome(&self, pos: [i32; 3]) -> Option<String> {
        if pos == self.below.pos {
            return self.below.biome.clone();
        }
        self.at
            .as_ref()
            .filter(|at| at.pos == pos)
            .and_then(|at| at.biome.clone())
    }
}
