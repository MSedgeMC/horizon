//! `Entity.saveWithoutId` writes `TicksFrozen` while it is positive
//! (`Entity.java:2148`) and `Entity.load` reads it (`Entity.java:2227`); only
//! `LivingEntity.aiStep` decrements it (`LivingEntity.java:3141`), so a
//! non-living projectile or experience orb keeps the value. Horizon modelled
//! the field on mobs, and for the kinds whose save is an `entity_nbt` writer
//! the loaded compound lost it (`remember_tags` kept no `TicksFrozen`).
use super::*;
use horizon_client::chunk::Chunk;
use horizon_core::position::ChunkPos;
use horizon_world::nbt::Tag;
use std::sync::Arc;

fn game() -> Game<'static> {
    let root = std::path::PathBuf::from(
        std::env::var_os("HORIZON_TEST_RESOURCES")
            .expect("set HORIZON_TEST_RESOURCES to prepared resources"),
    );
    let settings = Settings::default();
    let loaded = load_resources(
        root.clone(),
        None,
        &root,
        &settings,
        std::time::Instant::now(),
        false,
        true,
        &Progress::new(),
        &mut |_, _| unreachable!(),
    )
    .unwrap();
    let mut options = Options::parse(std::iter::empty()).unwrap().unwrap();
    options.resources = root.clone();
    options.execution_role = execution_role::ExecutionRole::IntegratedServer;
    let res: &'static _ = Box::leak(Box::new(loaded.resources));
    let mut game = Game::new(
        res,
        loaded.pack,
        loaded.gui.unwrap(),
        options,
        settings,
        Audio::silent(&root),
        None,
    )
    .unwrap();
    game.build_registries();
    game.set_allow_commands(true);
    game.world
        .chunks
        .insert((0, 0), Arc::new(Chunk::empty(0, 0)));
    game.block_ticks
        .add_empty_chunk(ChunkPos::new(0, 0))
        .unwrap();
    game
}

#[test]
#[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
fn ticks_frozen_survives_a_projectile_and_an_orbs_save_and_load() {
    let mut game = game();
    assert!(game.run_command("forceload add 0 0"));
    assert!(game.run_command("fill 0 199 0 15 199 15 stone"));
    assert!(game.run_command(
        "summon arrow 4.5 200 4.5 {TicksFrozen:60,NoGravity:1b,Motion:[0d,0d,0d],UUID:[I;169,0,0,1]}"
    ));
    assert!(game.run_command(
        "summon experience_orb 8.5 200 4.5 {TicksFrozen:60,NoGravity:1b,Motion:[0d,0d,0d],Value:1s,UUID:[I;169,0,0,2]}"
    ));
    let arrow = game.creatures.projectiles[0].id;
    let orb = game.orbs[0].id;
    for target in [
        commands::Target::Projectile(arrow),
        commands::Target::Orb(orb),
    ] {
        let tag = game.entity_nbt(target).expect("a live entity");
        assert_eq!(tag.get("TicksFrozen"), Some(&Tag::Int(60)), "{target:?}");
    }
    // The compounds a save writes.
    let keys = [(0, 0)].into_iter().collect();
    let (key, saved_arrow) = game
        .save_projectile(0, &keys)
        .expect("arrow saved with its chunk");
    assert_eq!(key, (0, 0));
    assert_eq!(saved_arrow.get("TicksFrozen"), Some(&Tag::Int(60)));
    let (key, saved_orb) = game.save_orb(0, &keys).expect("orb saved");
    assert_eq!(key, (0, 0));
    assert_eq!(saved_orb.get("TicksFrozen"), Some(&Tag::Int(60)));
    // `Entity.load` of each written compound.
    game.creatures.projectiles.clear();
    game.orbs.clear();
    game.load_arrow(saved_arrow, None, None);
    game.load_orb(saved_orb, None);
    let arrow = game.creatures.projectiles[0].id;
    let orb = game.orbs[0].id;
    for target in [
        commands::Target::Projectile(arrow),
        commands::Target::Orb(orb),
    ] {
        let tag = game.entity_nbt(target).expect("a live entity");
        assert_eq!(tag.get("TicksFrozen"), Some(&Tag::Int(60)), "{target:?}");
    }
    // Vanilla thaws only a living entity, so a projectile and an orb keep 60
    // outside powder snow; Horizon now keeps the loaded field the same way.
    game.tick().unwrap();
    for target in [
        commands::Target::Projectile(arrow),
        commands::Target::Orb(orb),
    ] {
        let tag = game.entity_nbt(target).expect("still live after a tick");
        assert_eq!(tag.get("TicksFrozen"), Some(&Tag::Int(60)), "{target:?}");
    }
}
