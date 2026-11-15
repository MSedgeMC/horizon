//! More than one ender dragon in a level: vanilla has no "one dragon" slot.
//! `/summon ender_dragon` twice, both joining, both ticking, both saved.
use super::super::{Audio, Game, Options, Progress, Settings, execution_role, load_resources};
use horizon_client::chunk::Chunk;
use std::collections::HashSet;
use std::sync::Arc;

fn with_game(test: impl FnOnce(&mut Game<'_>)) {
    let root = std::path::PathBuf::from(std::env::var_os("HORIZON_TEST_RESOURCES").unwrap());
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
    let mut game = Game::new(
        &loaded.resources,
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
    test(&mut game);
}

/// The dragons saved with the one loaded chunk.
fn saved_dragons(game: &mut Game<'_>) -> usize {
    let keys: HashSet<(i32, i32)> = [(0, 0)].into_iter().collect();
    game.entity_chunks(&keys, false)
        .iter()
        .flat_map(|(_, tags)| tags)
        .filter(|tag| {
            matches!(
                tag.get("id"),
                Some(horizon_world::nbt::Tag::String(id))
                    if id.to_string_lossy() == "minecraft:ender_dragon"
            )
        })
        .count()
}

/// A level holds as many dragons as `/summon` makes: the second joins
/// instead of being refused, both tick on their own, and the chunk's
/// entity list carries both. Only `EndDragonFight` tracks one, by UUID.
#[test]
#[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
fn a_second_summoned_dragon_joins_the_level_and_ticks() {
    with_game(|game| {
        for (n, y) in [80.0, 160.0].into_iter().enumerate() {
            assert!(
                game.run_command(&format!(
                    "summon minecraft:ender_dragon 4.5 {y} 4.5 {{NoAI:1b}}"
                )),
                "dragon {}: {:?}",
                n + 1,
                game.ui.chat.recent_lines(3)
            );
        }
        assert_eq!(saved_dragons(game), 2, "both dragons joined");

        let before = game.dragon.as_ref().expect("a current dragon").ticks;
        for _ in 0..20 {
            game.tick_server(false).unwrap();
        }
        assert!(
            game.dragon.as_ref().expect("a current dragon").ticks > before,
            "the current dragon did not tick"
        );
        assert_eq!(
            saved_dragons(game),
            2,
            "a tick must keep both dragons in the level"
        );
    });
}
