//! The creaking heart block (`CreakingHeartBlock`): its state follows the
//! pale oak logs on both ends of its axis and the `creaking_active`
//! environment attribute. The block entity (protector, particles, resin)
//! lives with the entities in the client.
use super::{Behaviours, LevelAccess, Sim, Ticks, update::rel};
use crate::registry::BlockStateId;
use horizon_core::{geometry::Direction, position::BlockPos};

/// `Direction.Axis.getDirections()`.
fn axis_directions(axis: &str) -> [Direction; 2] {
    match axis {
        "x" => [Direction::East, Direction::West],
        "z" => [Direction::South, Direction::North],
        _ => [Direction::Up, Direction::Down],
    }
}

impl<L: LevelAccess + ?Sized> Sim<'_, L> {
    /// `EnvironmentAttributes.CREAKING_ACTIVE`: the overworld day timeline
    /// turns it on from 12600 to 23400.
    pub(crate) fn creaking_active(&self) -> bool {
        self.level.dimension_type().has_timeline("minecraft:day")
            && (12600..23401).contains(&self.level.day_time().rem_euclid(24000))
    }
    /// `CreakingHeartBlock.hasRequiredLogs`: pale oak logs along the axis.
    pub(crate) fn heart_has_required_logs(&self, state: BlockStateId, pos: BlockPos) -> bool {
        let b = self.b;
        let axis = b.table.text(state, b.props.axis);
        axis_directions(axis).into_iter().all(|d| {
            let neighbour = self.state(rel(pos, d));
            b.in_set(neighbour, &b.tags.pale_oak_logs)
                && b.table.text(neighbour, b.props.axis) == axis
        })
    }
    /// `CreakingHeartBlock.updateState`: an uprooted heart with its logs wakes
    /// (at night) or goes dormant.
    pub(crate) fn heart_update_state(&self, state: BlockStateId, pos: BlockPos) -> BlockStateId {
        let b = self.b;
        let uprooted = b.table.text(state, b.props.creaking_heart_state) == "uprooted";
        if uprooted && self.heart_has_required_logs(state, pos) {
            let next = if self.creaking_active() {
                "awake"
            } else {
                "dormant"
            };
            b.table.set_text(state, b.props.creaking_heart_state, next)
        } else {
            state
        }
    }
    /// `CreakingHeartBlock.tick` (scheduled by `updateShape`).
    pub(crate) fn heart_tick(&mut self, state: BlockStateId, pos: BlockPos) {
        let next = self.heart_update_state(state, pos);
        if next != state {
            self.set_block_and_update(pos, next);
        }
    }
    /// `CreakingHeartBlock.getStateForPlacement` after the axis is set.
    pub(crate) fn heart_placement(&self, state: BlockStateId, pos: BlockPos) -> BlockStateId {
        self.heart_update_state(state, pos)
    }
}

impl Behaviours {
    /// `EnvironmentAttributes.CREAKING_ACTIVE` at the level's time.
    pub fn creaking_active<L: LevelAccess + ?Sized>(
        &self,
        level: &mut L,
        ticks: &mut Ticks,
    ) -> bool {
        self.sim(level, ticks).creaking_active()
    }
    /// `CreakingHeartBlock.hasRequiredLogs`.
    pub fn creaking_heart_has_logs<L: LevelAccess + ?Sized>(
        &self,
        level: &mut L,
        ticks: &mut Ticks,
        pos: BlockPos,
        state: BlockStateId,
    ) -> bool {
        self.sim(level, ticks).heart_has_required_logs(state, pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Direction.Axis.getDirections`: the positive direction first.
    #[test]
    fn axis_directions_are_positive_first() {
        assert_eq!(axis_directions("x"), [Direction::East, Direction::West]);
        assert_eq!(axis_directions("z"), [Direction::South, Direction::North]);
        assert_eq!(axis_directions("y"), [Direction::Up, Direction::Down]);
    }
}
