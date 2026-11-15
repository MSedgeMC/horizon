use super::*;
use std::io::Write;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "horizon-launcher-datapacks-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn zip_file(path: &Path, files: &[(&str, &str)]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, text) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

const META: &str = r#"{"pack":{"pack_format":88,"description":"More trades"}}"#;

#[test]
fn lists_adds_and_removes_as_the_game_reads_them() {
    let root = scratch("all");
    let world = root.join("saves/My World");
    zip_file(
        &world.join("datapacks/Trades.zip"),
        &[
            ("pack.mcmeta", META),
            ("data/t/function/a.mcfunction", "say hi"),
        ],
    );
    fs::create_dir_all(world.join("datapacks/folder pack")).unwrap();
    fs::write(
        world.join("datapacks/folder pack/pack.mcmeta"),
        r#"{"pack":{"description":{"text":"Mine"}}}"#,
    )
    .unwrap();
    fs::write(world.join("datapacks/readme.txt"), "").unwrap();
    fs::write(
        world.join("datapacks.json"),
        r#"{"enabled":[],"disabled":["file/folder pack","file/Gone.zip"]}"#,
    )
    .unwrap();
    let packs = list(&world);
    let seen: Vec<(&str, &str, bool)> = packs
        .iter()
        .map(|p| (p.file.as_str(), p.description.as_str(), p.enabled))
        .collect();
    assert_eq!(
        seen,
        [
            ("folder pack", "Mine", false),
            ("Trades.zip", "More trades", true)
        ]
    );

    let drops = root.join("drops");
    zip_file(
        &drops.join("Gone.zip"),
        &[("pack.mcmeta", META), ("data/g/tags/x.json", "{}")],
    );
    zip_file(
        &drops.join("Trades.zip"),
        &[("pack.mcmeta", META), ("data/t/function/b.mcfunction", "")],
    );
    zip_file(
        &drops.join("Textures.zip"),
        &[
            ("pack.mcmeta", META),
            ("assets/minecraft/textures/a.png", ""),
        ],
    );
    // A pack disabled before it was taken out comes back on.
    let gone = add(&world, &drops.join("Gone.zip")).unwrap();
    assert!(gone.enabled);
    assert!(!fs::read_to_string(world.join("datapacks.json"))
        .unwrap()
        .contains("Gone.zip"));
    assert_eq!(
        add(&world, &drops.join("Trades.zip")).unwrap().file,
        "Trades (2).zip"
    );
    let refused = add(&world, &drops.join("Textures.zip")).unwrap_err();
    assert!(refused.contains("resource pack"), "{refused}");

    let folder = find(&world, "folder pack").unwrap();
    let trash = root.join("trash");
    std::env::set_var(crate::desktop_seams::TRASH_ENV, &trash);
    remove(&world, &folder).unwrap();
    std::env::remove_var(crate::desktop_seams::TRASH_ENV);
    assert!(trash.join("folder pack/pack.mcmeta").is_file());
    assert!(!fs::read_to_string(world.join("datapacks.json"))
        .unwrap()
        .contains("folder pack"));
    assert_eq!(find(&world, "Trades").unwrap().file, "Trades.zip");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn coverage_says_what_runs() {
    let root = scratch("coverage");
    let file = root.join("p.zip");
    zip_file(
        &file,
        &[
            ("pack.mcmeta", META),
            ("data/a/function/x.mcfunction", ""),
            ("data/a/predicate/p.json", "{}"),
            ("data/a/item_modifier/m.json", "{}"),
            ("data/a/tags/item/i.json", "{}"),
            ("data/a/loot_table/l.json", "{}"),
            ("data/a/loot_table/k.json", "{}"),
            ("data/a/worldgen/biome/b.json", "{}"),
            ("data/a/worldgen/world_preset/w.json", "{}"),
            ("assets/a/lang/en_us.json", "{}"),
        ],
    );
    let c = coverage(&file).unwrap();
    assert_eq!(
        c.summary(),
        "Runs its 1 function, 1 item tag, 1 item modifier, 2 loot tables, 1 predicate, 1 \
         worldgen. Not read yet: 1 worldgen world preset."
    );
    let many = Coverage {
        read: Vec::new(),
        unread: (1..=8).map(|n| (format!("kind{n}"), n)).collect(),
    };
    assert_eq!(
        many.not_read(),
        "8 kind8s, 7 kind7s, 6 kind6s, 5 kind5s, 4 kind4s, 3 kind3s and 2 other kinds"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// `datapack::discover_folder`'s `folder_metadata_present` and vanilla's `Pack.readPackMetadata`
/// (whose fallback codec keeps only `description`): a folder without a `pack.description` is not
/// a pack either reader takes. Run against the real 26.3 codec: `{"pack":{"description":"ok"}}`
/// parses without any format field, while `{}`, `{"pack":{}}` and
/// `{"pack":{"description":null}}` do not.
#[test]
fn a_folder_without_a_description_is_not_a_pack() {
    let root = scratch("folder-description");
    let world = root.join("saves/W");
    for (name, meta) in [
        ("Empty", r#"{}"#),
        ("NoSection", r#"{"pack":{}}"#),
        ("Null", r#"{"pack":{"description":null}}"#),
        ("Mine", r#"{"pack":{"description":"Mine"}}"#),
    ] {
        fs::create_dir_all(world.join("datapacks").join(name)).unwrap();
        fs::write(world.join(format!("datapacks/{name}/pack.mcmeta")), meta).unwrap();
    }
    let names: Vec<String> = list(&world).into_iter().map(|p| p.file).collect();
    assert_eq!(names, ["Mine"]);
    fs::remove_dir_all(&root).unwrap();
}
