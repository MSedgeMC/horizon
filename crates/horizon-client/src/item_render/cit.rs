//! OptiFine Custom Item Textures on item models: the layer of a stack a `type=item` rule matches
//! draws the rule's model, or its own model with the rule's textures in some slots. The rules'
//! JSON models join the model library, their textures the atlas (`cit_assets`).

use super::{ItemRenderer, corners};
use crate::cit::{CitRules, Kind};
use crate::cit_assets::{self, model_id, sprite_id};
use crate::hud::cit_glint::GlintTextures;
use glam::Vec3;
use horizon_assets::{
    item_bake::{BakedItemModel, bake_resolved},
    model::{Material, TextureSlot},
    resolve::{MISSING_MODEL, ModelLibrary},
};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

/// A baked layer and its quad corners, as `ItemRenderer::bake` and `quad_extents` give them.
pub(super) type Baked = (Arc<BakedItemModel>, Arc<Vec<Vec3>>);

/// What one item rule swaps, as model and sprite ids.
#[derive(Debug, Default)]
struct Plan {
    model: Option<String>,
    /// `model.<name>`, by the leaf model's [`short`] name.
    models: HashMap<String, String>,
    texture: Option<String>,
    /// `texture.<name>`: a leaf, a slot or a sprite's file name, and its sprite.
    textures: Vec<(String, String)>,
}

/// One swapped bake: the model and the sprite each changed slot takes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CitBake {
    base: String,
    slots: BTreeMap<String, String>,
}

/// The enabled packs' rules and what their item rules bake to.
#[derive(Debug)]
pub struct CitModels {
    pub rules: CitRules,
    /// The enchantment rules' glint textures.
    pub glint_textures: GlintTextures,
    /// Per rule (an index into `rules.rules`); `None` for rules of other kinds.
    plans: Vec<Option<Plan>>,
    /// Per (rule, leaf model): its bake, `None` where the rule leaves the leaf alone.
    memo: Mutex<HashMap<(usize, String), Option<CitBake>>>,
    baked: Mutex<HashMap<CitBake, Baked>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The last segment of an id: `minecraft:item/bow_pulling_1` is `bow_pulling_1`.
fn short(id: &str) -> &str {
    let path = id.rsplit_once(':').map_or(id, |(_, p)| p);
    path.rsplit('/').next().unwrap_or(path)
}

/// The rules at `root` and `library` with their models, or `library` alone without rules.
pub(super) fn load(
    root: &Path,
    library: Arc<ModelLibrary>,
) -> (Arc<ModelLibrary>, Option<CitModels>) {
    let rules = CitRules::load(root);
    // A `cit.properties` alone can still turn vanilla's glint off.
    if rules.is_empty() && rules.global.use_glint {
        return (library, None);
    }
    let extra = cit_assets::load_models(root, &rules.rules);
    let library = if extra.is_empty() {
        library
    } else {
        match library.overlaid(extra) {
            Ok(over) => Arc::new(over),
            Err(e) => {
                log::warn!("custom item textures: models left out: {e}");
                library
            }
        }
    };
    let mut models = CitModels::new(rules);
    models.glint_textures = GlintTextures::load(root, &models.rules.rules);
    (library, Some(models))
}

impl CitModels {
    pub fn new(rules: CitRules) -> Self {
        let pairs = |list: &[(String, String)], id: fn(&str) -> Option<String>| {
            list.iter()
                .filter_map(|(name, path)| Some((short(name).to_owned(), id(path)?)))
                .collect::<Vec<_>>()
        };
        let plans = rules
            .rules
            .iter()
            .map(|r| {
                (r.kind == Kind::Item).then(|| Plan {
                    model: r.model.as_deref().and_then(model_id),
                    models: pairs(&r.models, model_id).into_iter().collect(),
                    texture: r.texture.as_deref().and_then(sprite_id),
                    textures: pairs(&r.textures, sprite_id),
                })
            })
            .collect();
        Self {
            rules,
            glint_textures: GlintTextures::default(),
            plans,
            memo: Mutex::default(),
            baked: Mutex::default(),
        }
    }
}

impl ItemRenderer {
    /// The layer whose model is `leaf` as item rule `rule` draws it; `None` draws it as usual.
    pub(super) fn cit_layer(&self, rule: usize, leaf: &str) -> Option<Baked> {
        let cit = self.cit.as_ref()?;
        let key = (rule, leaf.to_owned());
        let known = lock(&cit.memo).get(&key).cloned();
        let bake = match known {
            Some(bake) => bake,
            None => {
                let bake = self.cit_plan(cit, rule, leaf);
                lock(&cit.memo).insert(key, bake.clone());
                bake
            }
        }?;
        if let Some(hit) = lock(&cit.baked).get(&bake) {
            return Some(hit.clone());
        }
        let baked = self.cit_bake(&bake);
        lock(&cit.baked).insert(bake, baked.clone());
        Some(baked)
    }

    /// CIT Resewn's `CITItem` model choice for one leaf: `model.<leaf>` draws that model;
    /// `texture.<leaf>` the leaf with it as `layer0`; else `model` (or the leaf) with `texture`
    /// in every slot of a model, or as the leaf's `layer0`. Then each `texture.<name>` goes to
    /// the slot `name` and to every slot whose sprite's file is `name` (`potion_overlay`).
    fn cit_plan(&self, cit: &CitModels, rule: usize, leaf: &str) -> Option<CitBake> {
        let plan = cit.plans.get(rule)?.as_ref()?;
        let sub = short(leaf);
        if let Some(model) = plan.models.get(sub) {
            return Some(CitBake {
                base: model.clone(),
                slots: BTreeMap::new(),
            });
        }
        let named = plan.textures.iter().find(|(n, _)| n == sub).map(|(_, s)| s);
        let base = match (named, &plan.model) {
            (None, Some(model)) => model.clone(),
            _ => leaf.to_owned(),
        };
        let textures = self
            .library
            .resolve(&base)
            .map(|r| r.textures)
            .unwrap_or_default();
        let mut slots = BTreeMap::new();
        match (named, &plan.texture) {
            (Some(sprite), _) => {
                slots.insert("layer0".to_owned(), sprite.clone());
            }
            (None, Some(sprite)) if plan.model.is_some() => {
                for slot in textures.keys() {
                    slots.insert(slot.clone(), sprite.clone());
                }
            }
            (None, Some(sprite)) => {
                slots.insert("layer0".to_owned(), sprite.clone());
            }
            (None, None) => {}
        }
        for (name, sprite) in &plan.textures {
            for (slot, material) in &textures {
                if slot == name || short(&material.sprite) == name {
                    slots.insert(slot.clone(), sprite.clone());
                }
            }
        }
        (base != leaf || !slots.is_empty()).then_some(CitBake { base, slots })
    }

    fn cit_bake(&self, bake: &CitBake) -> Baked {
        let child = bake
            .slots
            .iter()
            .map(|(slot, sprite)| {
                let material = Material {
                    sprite: sprite.clone(),
                    force_translucent: false,
                };
                (slot.clone(), TextureSlot::Material(material))
            })
            .collect();
        let baked = self
            .library
            .resolve_with_textures(&bake.base, &child)
            .and_then(|model| bake_resolved(&model, &self.sprites))
            .map(Arc::new)
            .unwrap_or_else(|_| self.bake(MISSING_MODEL));
        let extents = Arc::new(corners(&baked));
        (baked, extents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_assets::item_model::{DisplayContext, NoEnv, StackView};
    use std::{collections::BTreeSet, fs, path::PathBuf};

    const CIT: &str = "assets/minecraft/optifine/cit";

    fn png(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = std::io::BufWriter::new(fs::File::create(path).unwrap());
        let mut encoder = png::Encoder::new(file, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255; 16]).unwrap();
        writer.finish().unwrap();
    }

    /// A stick, a bow's frames and a potion; with `cit`, rules over them.
    fn root(name: &str, cit: bool) -> PathBuf {
        let root = std::env::temp_dir().join(format!("horizon-cit-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write(
            "items.json",
            r#"{"minecraft:stick":{"minecraft:item_model":"minecraft:stick"}}"#,
        );
        write(
            "assets/minecraft/items/stick.json",
            r#"{"model":{"type":"minecraft:model","model":"minecraft:item/stick"}}"#,
        );
        let models = "assets/minecraft/models/item";
        write(
            &format!("{models}/generated.json"),
            r#"{"parent":"builtin/generated"}"#,
        );
        let flat = |name: &str, layers: &[&str]| {
            let textures: Vec<String> = layers
                .iter()
                .enumerate()
                .map(|(i, l)| format!(r#""layer{i}":"item/{l}""#))
                .collect();
            write(
                &format!("{models}/{name}.json"),
                &format!(
                    r#"{{"parent":"item/generated","textures":{{{}}}}}"#,
                    textures.join(",")
                ),
            );
            for l in layers {
                png(&root.join(format!("assets/minecraft/textures/item/{l}.png")));
            }
        };
        flat("stick", &["stick"]);
        flat("bow", &["bow"]);
        flat("bow_pulling_0", &["bow_pulling_0"]);
        flat("bow_pulling_1", &["bow_pulling_1"]);
        flat("potion", &["potion_overlay", "potion"]);
        if cit {
            for t in ["a", "b", "blade", "hilt"] {
                png(&root.join(format!("{CIT}/{t}.png")));
            }
            write(
                &format!("{CIT}/sword.json"),
                r#"{"parent":"item/generated","textures":{"layer0":"./blade","layer1":"./hilt"}}"#,
            );
            write(
                &format!("{CIT}/other.json"),
                r#"{"parent":"item/generated","textures":{"layer0":"./hilt"}}"#,
            );
            let rules = [
                ("plain", "items=bow\ntexture.bow_pulling_1=a"),
                (
                    "mix",
                    "items=bow\nmodel=sword\ntexture=a\ntexture.bow_pulling_1=b\nmodel.bow_pulling_0=other",
                ),
                (
                    "potion",
                    "items=potion\ntexture.potion_overlay=a\ntexture.layer1=b",
                ),
                ("stick", "items=stick\nmodel=sword"),
            ];
            for (name, text) in rules {
                write(&format!("{CIT}/{name}.properties"), text);
            }
        }
        root
    }

    fn renderer(root: &Path) -> ItemRenderer {
        let library = crate::model_library::ParsedModels::load(root).into_item_library();
        ItemRenderer::load_with_library(root, library).unwrap()
    }

    fn rule(r: &ItemRenderer, name: &str) -> usize {
        let path = format!("{CIT}/{name}.properties");
        let rules = &r.cit.as_ref().unwrap().rules.rules;
        rules.iter().position(|r| r.path == path).unwrap()
    }

    fn sprites(baked: &BakedItemModel) -> BTreeSet<String> {
        baked
            .quads
            .iter()
            .map(|q| q.material.sprite.clone())
            .collect()
    }

    fn cit(name: &str) -> String {
        format!("horizon_cit:minecraft/optifine/cit/{name}")
    }

    #[test]
    fn rules_pick_a_model_or_textures_per_leaf() {
        let root = root("plans", true);
        let r = renderer(&root);
        let layer = |rule: &str, leaf: &str| {
            r.cit_layer(self::rule(&r, rule), &format!("minecraft:item/{leaf}"))
                .map(|(baked, _)| sprites(&baked).into_iter().collect::<Vec<_>>())
        };
        // `texture.<leaf>` swaps that leaf only.
        assert_eq!(layer("plain", "bow_pulling_1"), Some(vec![cit("a")]));
        assert_eq!(layer("plain", "bow_pulling_0"), None);
        assert_eq!(layer("plain", "bow"), None);
        // `model.<leaf>` first, then `texture.<leaf>`, then `model` with `texture` in every slot.
        assert_eq!(layer("mix", "bow_pulling_0"), Some(vec![cit("hilt")]));
        assert_eq!(layer("mix", "bow_pulling_1"), Some(vec![cit("b")]));
        assert_eq!(layer("mix", "bow"), Some(vec![cit("a")]));
        // A model alone keeps its own (relative) textures.
        assert_eq!(
            layer("stick", "stick"),
            Some(vec![cit("blade"), cit("hilt")])
        );
        // By sprite file name and by slot name.
        assert_eq!(layer("potion", "potion"), Some(vec![cit("a"), cit("b")]));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bake_is_shared_by_its_rule_and_leaf_only() {
        let root = root("cache", true);
        let r = renderer(&root);
        let leaf = "minecraft:item/bow_pulling_1";
        let (plain, mix) = (rule(&r, "plain"), rule(&r, "mix"));
        let first = r.cit_layer(plain, leaf).unwrap();
        let again = r.cit_layer(plain, leaf).unwrap();
        assert!(Arc::ptr_eq(&first.0, &again.0));
        assert!(Arc::ptr_eq(&first.1, &again.1));
        assert!(!Arc::ptr_eq(&first.0, &r.cit_layer(mix, leaf).unwrap().0));
        assert!(!Arc::ptr_eq(&first.0, &r.bake(leaf)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_matched_stack_resolves_to_the_rule_model() {
        let root = root("resolve", true);
        let r = renderer(&root);
        let view = StackView::new("minecraft:stick", 1);
        let resolve = |rule| {
            let state =
                r.resolve_with_rule(&view, DisplayContext::Gui, None, None, 0, &NoEnv, rule);
            match &state.layers[0].geometry {
                super::super::LayerGeometry::Quads(baked) => sprites(baked),
                other => panic!("{other:?}"),
            }
        };
        let stick = rule(&r, "stick");
        assert_eq!(
            resolve(Some(stick)),
            BTreeSet::from([cit("blade"), cit("hilt")])
        );
        assert_eq!(
            resolve(None),
            BTreeSet::from(["minecraft:item/stick".to_owned()])
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn without_rules_nothing_changes() {
        let root = root("none", false);
        let r = renderer(&root);
        assert!(r.cit.is_none());
        let view = StackView::new("minecraft:stick", 1);
        let state = |rule| {
            let s = r.resolve_with_rule(&view, DisplayContext::Gui, None, None, 0, &NoEnv, rule);
            format!("{s:?}")
        };
        let plain = format!(
            "{:?}",
            r.resolve(&view, DisplayContext::Gui, None, None, 0, &NoEnv)
        );
        assert_eq!(state(Some(0)), plain);
        assert_eq!(state(None), plain);
        assert!(plain.contains("minecraft:item/stick"));
        let _ = fs::remove_dir_all(&root);
    }
}
