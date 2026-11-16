//! `/serverpack` (ServerPackCommand): `push` offers every connection the
//! pack and `pop` takes it away, as `sendToAllConnections` does.
use super::*;
use crate::game::lan::LanSettings;
use horizon_client::{
    command::vanilla,
    lan::{Joined, NetConfig, Out, PlayerSender, guest::Guest, hub::HubConfig},
};
use horizon_protocol::packets::{
    configuration::{ClientInformation, ResourcePackPop, ResourcePackPush},
    play::Clientbound as C,
};
use std::sync::{Arc, atomic::AtomicI32, mpsc};

const CONN: u32 = 2;

/// A dedicated-server game with one placed guest and the channel its
/// connection writes to.
fn game_with_guest() -> (Game<'static>, mpsc::Receiver<Out>) {
    let mut game = crate::game::native_machines::tests::server_game();
    game.build_registries();
    game.set_allow_commands(true);
    game.lan_host(
        LanSettings {
            port: 0,
            guest_commands: true,
            force_game_mode: false,
        },
        NetConfig {
            announce: false,
            ..NetConfig::loopback(0, "serverpack-test".into(), 2)
        },
        HubConfig {
            default_game_mode: 0,
            guest_commands: true,
            force_game_mode: false,
            hardcore: false,
            view_distance: 2,
            simulation_distance: 2,
            dimension: "minecraft:overworld".into(),
            allow_flight: false,
            seed: 0,
            max_players: 2,
            integrated: false,
            levels: Default::default(),
            level_seeds: Default::default(),
        },
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let mut guest = Guest::new(
        Joined {
            conn: CONN,
            uuid: 424,
            name: "PackGuest".into(),
            properties: Vec::new(),
            info: ClientInformation::new(2),
            brand: "test".into(),
            sender: PlayerSender::new(tx, Arc::default()),
            latency: Arc::new(AtomicI32::new(0)),
            address: "127.0.0.1:25565".parse().unwrap(),
            expected: false,
        },
        CONN as i32,
        [8.5, 64.0, 8.5],
        0,
    );
    guest.dimension = "minecraft:overworld".into();
    guest.placed = true;
    guest.loaded = true;
    game.lan.as_mut().unwrap().hub.guests.push(guest);
    (game, rx)
}

/// Runs a development command through the development tree, as the server
/// does, at the game master level the dev commands require.
fn dev_command(game: &mut Game<'_>, command: &str) {
    game.ui.registries.permission = 4;
    let d = vanilla::build_with(true);
    let parse = d.parse(command, &game.ui.registries);
    let (ctx, id) = parse.executable().unwrap();
    let (ctx, id) = (ctx.clone(), id.to_owned());
    game.execute_command(&id, &ctx).unwrap();
}

/// The packet bodies the guest was sent.
fn sent(rx: &mpsc::Receiver<Out>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while let Ok(Out::Packet(body)) = rx.try_recv() {
        out.push(body);
    }
    out
}

#[test]
#[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
fn serverpack_push_and_pop_reach_every_connection() {
    let (mut game, rx) = game_with_guest();
    let url = "https://example.com/pack.zip";

    // No uuid: UUID.nameUUIDFromBytes(url), no hash.
    dev_command(&mut game, &format!("serverpack push {url}"));
    assert_eq!(
        sent(&rx),
        [C::ResourcePackPush(ResourcePackPush {
            id: horizon_server_files::properties::name_uuid(url.as_bytes()),
            url: url.into(),
            hash: String::new(),
            required: false,
            prompt: None,
        })
        .encode()]
    );

    // The explicit uuid and hash are the packet's.
    let uuid = "123e4567-e89b-12d3-a456-426614174000";
    let id = 0x123e_4567_e89b_12d3_a456_4266_1417_4000;
    dev_command(&mut game, &format!("serverpack push {url} {uuid} deadbeef"));
    assert_eq!(
        sent(&rx),
        [C::ResourcePackPush(ResourcePackPush {
            id,
            url: url.into(),
            hash: "deadbeef".into(),
            required: false,
            prompt: None,
        })
        .encode()]
    );

    // Pop names that pack.
    dev_command(&mut game, &format!("serverpack pop {uuid}"));
    assert_eq!(
        sent(&rx),
        [C::ResourcePackPop(ResourcePackPop { id: Some(id) }).encode()]
    );
}
