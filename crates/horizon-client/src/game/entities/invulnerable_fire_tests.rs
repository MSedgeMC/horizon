//! `Invulnerable` refuses fire damage, and `NoAI` does not stop `Witch.aiStep`.
//!
//! `Entity.isInvulnerableToBase` refuses the `hurtServer` calls that
//! `Entity.baseTick` and `ArmorStand.hurtServer` make, so a burning
//! `Invulnerable` armor stand or experience orb keeps its health. `NoAI` only
//! skips `serverAiStep` (`Mob.isEffectiveAi`), so `Witch.aiStep` still runs
//! and picks its fire-resistance potion while it burns.
use super::*;
use horizon_client::mobs::Species;
use horizon_core::random::{EntityRandom, LegacyRandom};

fn step(game: &mut Game<'_>) {
    assert!(game.run_command("tick step 1"));
    game.tick_server(false).unwrap();
}

#[test]
#[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
fn invulnerable_burning_entities_keep_health_and_a_noai_witch_picks_fire_resistance() {
    let mut game = crate::game::native_machines::tests::server_game();
    game.build_registries();
    game.set_allow_commands(true);
    assert!(game.run_command("difficulty normal"));
    assert!(game.run_command("forceload add 0 0"));
    assert!(game.run_command("tick freeze"));
    assert!(
        game.run_command("summon armor_stand 8.5 200 8.5 {Fire:300s,NoGravity:1b,Invulnerable:1b}")
    );
    assert!(game.run_command(
        "summon experience_orb 6.5 200 8.5 {Fire:300s,NoGravity:1b,Invulnerable:1b,Value:1s}"
    ));
    assert!(
        game.run_command(
            "summon witch 10.5 200 8.5 {Fire:300s,NoAI:1b,NoGravity:1b,Invulnerable:1b}"
        )
    );
    // Entity.baseTick / ArmorStand.hurtServer hurt at `Fire % 20 == 0`, which
    // 300 is: the first tick would take 4 from the stand and 1 from the orb.
    for _ in 0..20 {
        step(&mut game);
    }
    assert_eq!(
        game.armor_stands.first().map(|a| a.health),
        Some(20.0),
        "an Invulnerable armor stand on fire keeps its health"
    );
    assert_eq!(
        game.orbs.first().map(|o| o.health),
        Some(5),
        "an Invulnerable experience orb on fire keeps its health"
    );
    // A NoAI witch still runs Witch.aiStep, whose 15% fire-resistance draw
    // picks the potion. Each seed starts the tick draws afresh; before the
    // fix no seed ran `ai_step` at all, so no iteration below could pick.
    let witch = game
        .mobs
        .iter()
        .position(|m| m.species() == Species::Witch)
        .expect("the summoned witch");
    let mut picked = None;
    for seed in 0..64 {
        {
            let m = &mut game.mobs[witch];
            m.extra.set_main_hand(None);
            m.extra.flag = false;
            m.extra.using_item = 0;
            m.fire_ticks = 300;
            m.random = EntityRandom::own(LegacyRandom::new(seed));
        }
        step(&mut game);
        if let Some(item) = game.mobs[witch].extra.main_hand_id() {
            picked = Some(item.to_string());
            break;
        }
    }
    assert_eq!(
        picked.as_deref(),
        Some("minecraft:fire_resistance"),
        "a burning NoAI witch picks its fire-resistance potion into the mainhand"
    );
}
