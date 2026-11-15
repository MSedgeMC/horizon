//! #16909: the fire step of `Entity.baseTick` on the non-living kinds whose
//! live state Horizon keeps in their loaded compound.
use crate::game::Game;
use crate::game::native_machines::tests::server_game;
use horizon_client::chunk::Chunk;
use horizon_core::position::ChunkPos;
use horizon_world::nbt::Tag;
use std::sync::Arc;

/// A server game whose origin chunk ticks entities.
fn scene() -> Game<'static> {
    let mut game = server_game();
    game.build_registries();
    game.set_allow_commands(true);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    game.block_ticks
        .add_empty_chunk(ChunkPos::new(0, 0))
        .unwrap();
    game.load_ticking_neighbourhood(0, 0);
    assert!(game.run_command("forceload add 0 0"));
    assert!(
        game.entity_ticking_area()(4, 4),
        "the origin chunk ticks entities"
    );
    game
}

/// The `Fire` that `/data get` reads for the entity with `uuid`.
fn saved_fire(game: &mut Game<'_>, uuid: [i32; 4]) -> Option<i32> {
    let target = game.target_with_uuid(uuid)?;
    let tag = game.entity_nbt(target)?;
    match tag.get("Fire") {
        Some(Tag::Short(ticks)) => Some(i32::from(*ticks)),
        Some(Tag::Int(ticks)) => Some(*ticks),
        _ => None,
    }
}

/// #16909, the `Fire` rows that share `Entity.baseTick`: a potion cloud is fire
/// immune and clears its loaded timer on the first tick; an evoker fang, an eye
/// of ender and an experience orb count theirs down.
#[test]
#[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
fn a_loaded_nonliving_entity_clears_or_counts_down_its_fire() {
    let mut game = scene();
    for (uuid, kind, at) in [
        ([332, 0, 0, 1], "area_effect_cloud", "4.5 200 4.5"),
        ([332, 0, 0, 2], "evoker_fangs", "7.5 200 4.5"),
        ([332, 0, 0, 3], "eye_of_ender", "10.5 200 4.5"),
        ([332, 0, 0, 4], "experience_orb", "13.5 200 4.5"),
    ] {
        assert!(
            game.run_command(&format!(
                "summon {kind} {at} {{UUID:[I;332,0,0,{}],Fire:300s,NoGravity:1b}}",
                uuid[3]
            )),
            "{kind}"
        );
        assert_eq!(
            saved_fire(&mut game, uuid),
            Some(300),
            "{kind} keeps its loaded Fire"
        );
    }
    game.tick_world(true).unwrap();
    assert_eq!(
        saved_fire(&mut game, [332, 0, 0, 1]),
        Some(0),
        "an area effect cloud is fire immune"
    );
    assert_eq!(
        saved_fire(&mut game, [332, 0, 0, 2]),
        Some(299),
        "the fangs"
    );
    assert_eq!(saved_fire(&mut game, [332, 0, 0, 3]), Some(299), "the eye");
    assert_eq!(saved_fire(&mut game, [332, 0, 0, 4]), Some(299), "the orb");
    for _ in 0..19 {
        game.tick_world(true).unwrap();
    }
    assert_eq!(
        saved_fire(&mut game, [332, 0, 0, 4]),
        Some(280),
        "the orb after 20 ticks, as the issue measures"
    );
}
