//! Custom Item Textures' enchantment glints (`type=enchantment`): which of a stack's matching
//! rules draw, in which order and how strongly, and whether vanilla's glint still draws under
//! them. The pack's `cit.properties` (`Global`) says how layers share an item.
//!
//! Written from OptiFine's `cit.properties` doc and CIT Resewn's (MIT) `TypeEnchantment` of
//! before its 1.21.11 port, which implemented `method`, `cap` and `fade`: the sums and the
//! cycle's fading follow it, except where noted.

pub use horizon_optifine::cit::{Global, Method};

use horizon_optifine::Properties;
use horizon_optifine::cit::Rule;
use std::path::Path;

/// The pack's global `cit.properties` under the overlay resource root: the first of
/// `Global::PATHS` that exists, else the defaults.
pub fn load_global(root: &Path) -> Global {
    Global::PATHS
        .iter()
        .find_map(|path| std::fs::read(root.join(path)).ok())
        .map(|bytes| {
            // Java reads `.properties` as Latin-1.
            let text = String::from_utf8(bytes)
                .unwrap_or_else(|e| e.into_bytes().iter().map(|&b| char::from(b)).collect());
            Global::parse(&Properties::parse_equals_first(&text))
        })
        .unwrap_or_default()
}

/// One glint layer a stack draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlintLayer {
    /// The rule, an index into `CitRules::rules`.
    pub rule: usize,
    /// Its `average` or `layered` share, 1 under `cycle`.
    pub weight: f32,
    /// Its `duration`, for `cycle`.
    duration: f32,
}

/// What a stack's glint is made of, worked out when the stack changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackGlint {
    /// Bottom first (`layer` ascending).
    pub layers: Vec<GlintLayer>,
    /// Whether vanilla's glint draws (on a stack that has one) under the layers.
    pub vanilla: bool,
    /// `fade` when the layers take turns (`method=cycle`).
    cycle_fade: Option<f32>,
}

impl StackGlint {
    /// The layers among `matched` (the stack's matching enchantment rules, in priority order,
    /// as `CitRules::enchantments` gives them) that draw, for a stack with `enchantments`
    /// (`ItemStack::crafting_enchantments`).
    pub fn new(
        rules: &[Rule],
        matched: &[usize],
        enchantments: &[(impl AsRef<str>, u32)],
        global: &Global,
    ) -> Self {
        // The highest priority rule of each layer.
        let mut layers: Vec<usize> = Vec::new();
        for &i in matched {
            let layer = rules[i].glint.layer;
            if !layers.iter().any(|&j| rules[j].glint.layer == layer) {
                layers.push(i);
            }
        }
        layers.sort_by_key(|&i| rules[i].glint.layer);
        // CIT Resewn: any matched layer asking for vanilla's glint keeps it, and the global
        // `useGlint=false` removes it everywhere, as OptiFine's doc says too.
        let vanilla = global.use_glint
            && (layers.is_empty() || layers.iter().any(|&i| rules[i].glint.use_glint));
        let cycle = global.method == Method::Cycle;
        // OptiFine's doc: `cap` is for `average` and `layered`, and keeps the topmost layers.
        // (CIT Resewn keeps the bottom ones, and caps cycles too.)
        if let Some(cap) = global.cap.filter(|_| !cycle) {
            let cap = usize::try_from(cap).unwrap_or(usize::MAX);
            layers = layers.split_off(layers.len().saturating_sub(cap));
        }
        // CIT Resewn: shares are against every enchantment of the stack, not only those the
        // layers name.
        let levels = enchantments.iter().map(|(_, level)| *level as f32);
        let total = match global.method {
            Method::Average => levels.sum::<f32>(),
            Method::Layered => levels.fold(0.0, f32::max),
            Method::Cycle => 0.0,
        };
        let layers = layers
            .into_iter()
            .map(|i| {
                let glint = &rules[i].glint;
                let named = &rules[i].enchantments;
                let weight = if cycle || named.is_empty() {
                    1.0
                } else {
                    // The highest level of the enchantments the rule names (CIT Resewn takes
                    // the first it finds; a rule rarely names two the stack has).
                    let level = enchantments
                        .iter()
                        .filter(|(id, _)| {
                            let id: &str = id.as_ref();
                            named.iter().any(|n| n == id)
                        })
                        .map(|(_, level)| *level as f32)
                        .fold(0.0, f32::max);
                    if total > 0.0 { level / total } else { 0.0 }
                };
                GlintLayer {
                    rule: i,
                    weight,
                    duration: glint.duration,
                }
            })
            .collect();
        Self {
            layers,
            vanilla,
            cycle_fade: cycle.then_some(global.fade),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Layer `index`'s intensity `seconds` into the game: its weight, and under `method=cycle`
    /// its turn.
    pub fn intensity(&self, index: usize, seconds: f64) -> f32 {
        let layer = self.layers[index];
        match self.cycle_fade {
            None => layer.weight,
            Some(fade) => cycle_intensity(&self.layers, fade, index, seconds),
        }
    }
}

/// CIT Resewn's `Loops.statelessFadingLoop`: each layer shows alone for its `duration`, then
/// fades into the next over `fade` seconds, the last into the first.
fn cycle_intensity(layers: &[GlintLayer], fade: f32, index: usize, seconds: f64) -> f32 {
    let n = layers.len();
    let fade = f64::from(fade);
    let period: f64 = layers.iter().map(|l| f64::from(l.duration) + fade).sum();
    if n == 1 || period <= 0.0 {
        return if index == 0 { 1.0 } else { 0.0 };
    }
    let mut t = seconds.rem_euclid(period);
    for (i, layer) in layers.iter().enumerate() {
        let duration = f64::from(layer.duration);
        if t < duration {
            return if i == index { 1.0 } else { 0.0 };
        }
        t -= duration;
        if t < fade {
            let into = (t / fade) as f32;
            return if i == index {
                1.0 - into
            } else if (i + 1) % n == index {
                into
            } else {
                0.0
            };
        }
        t -= fade;
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "assets/minecraft/optifine/cit";

    /// An enchantment rule named `name` with `text` after `type` and `texture`.
    fn rule(name: &str, text: &str) -> Rule {
        let text = format!("type=enchantment\ntexture=glint\n{text}");
        let path = format!("{DIR}/{name}.properties");
        Rule::parse(&path, &Properties::parse_equals_first(&text), |_| true).unwrap()
    }

    fn global(text: &str) -> Global {
        Global::parse(&Properties::parse_equals_first(text))
    }

    fn rules_of(layers: &StackGlint) -> Vec<usize> {
        layers.layers.iter().map(|l| l.rule).collect()
    }

    const SWORD: [(&str, u32); 2] = [("minecraft:sharpness", 3), ("minecraft:unbreaking", 1)];

    #[test]
    fn one_rule_per_layer_bottom_first() {
        let rules = [
            rule("a", "layer=2"),
            rule("b", "layer=0"),
            rule("c", "layer=2"),
            rule("d", "layer=-1"),
        ];
        // Priority order a, b, c, d: c loses layer 2 to a.
        let g = StackGlint::new(&rules, &[0, 1, 2, 3], &SWORD, &Global::default());
        assert_eq!(rules_of(&g), [3, 1, 0]);
        let g = StackGlint::new(&rules, &[2, 0], &SWORD, &Global::default());
        assert_eq!(rules_of(&g), [2]);
    }

    #[test]
    fn average_and_layered_shares() {
        let rules = [
            rule("sharp", "enchantments=sharpness smite"),
            rule("unb", "layer=1\nenchantments=unbreaking"),
            rule("any", "layer=2"),
            rule("fire", "layer=3\nenchantments=fire_aspect"),
        ];
        let weights = |method: &str| {
            let g = StackGlint::new(&rules, &[0, 1, 2, 3], &SWORD, &global(method));
            g.layers.iter().map(|l| l.weight).collect::<Vec<_>>()
        };
        // Sharpness III and Unbreaking I: 3 / 4 and 1 / 4; against the highest, 3 / 3 and 1 / 3.
        // A rule naming no enchantment is at full strength, one the stack lacks at none.
        assert_eq!(weights("method=average"), [0.75, 0.25, 1.0, 0.0]);
        assert_eq!(weights("method=layered"), [1.0, 1.0 / 3.0, 1.0, 0.0]);
        assert_eq!(weights("method=cycle"), [1.0; 4]);
        // An unenchanted stack: nothing to share.
        let g = StackGlint::new(&rules, &[0, 2], &[] as &[(&str, u32)], &Global::default());
        assert_eq!(g.layers[0].weight, 0.0);
        assert_eq!(g.layers[1].weight, 1.0);
    }

    #[test]
    fn cap_keeps_the_topmost_layers() {
        let rules = [
            rule("a", "layer=0"),
            rule("b", "layer=1"),
            rule("c", "layer=2"),
        ];
        let g = StackGlint::new(&rules, &[0, 1, 2], &SWORD, &global("cap=2"));
        assert_eq!(rules_of(&g), [1, 2]);
        let g = StackGlint::new(&rules, &[0, 1, 2], &SWORD, &global("cap=0"));
        assert!(g.is_empty());
        // Cycles show every layer in turn.
        let g = StackGlint::new(&rules, &[0, 1, 2], &SWORD, &global("cap=1\nmethod=cycle"));
        assert_eq!(rules_of(&g), [0, 1, 2]);
    }

    #[test]
    fn vanilla_glint() {
        let plain = rule("plain", "");
        let keep = rule("keep", "layer=1\nuseGlint=true");
        let rules = [plain, keep];
        let vanilla = |matched: &[usize], text: &str| {
            StackGlint::new(&rules, matched, &SWORD, &global(text)).vanilla
        };
        assert!(vanilla(&[], ""), "no rule: vanilla's glint");
        assert!(!vanilla(&[0], ""), "a rule replaces it");
        assert!(vanilla(&[0, 1], ""), "unless one keeps it");
        assert!(!vanilla(&[], "useGlint=false"));
        assert!(!vanilla(&[1], "useGlint=false"), "the pack's word is final");
    }

    #[test]
    fn cycles_take_turns_and_fade() {
        let rules = [
            rule("a", "duration=2"),
            rule("b", "layer=1\nduration=1"),
            rule("c", "layer=2\nduration=1"),
        ];
        let g = StackGlint::new(
            &rules,
            &[0, 1, 2],
            &SWORD,
            &global("method=cycle\nfade=0.5"),
        );
        // a 0-2, a→b 2-2.5, b 2.5-3.5, b→c 3.5-4, c 4-5, c→a 5-5.5, then again.
        let at = |t: f64| [0, 1, 2].map(|i| g.intensity(i, t));
        assert_eq!(at(1.0), [1.0, 0.0, 0.0]);
        assert_eq!(at(2.125), [0.75, 0.25, 0.0]);
        assert_eq!(at(3.0), [0.0, 1.0, 0.0]);
        assert_eq!(at(3.75), [0.0, 0.5, 0.5]);
        assert_eq!(at(4.5), [0.0, 0.0, 1.0]);
        assert_eq!(at(5.25), [0.5, 0.0, 0.5]);
        assert_eq!(at(5.5 + 3.0), [0.0, 1.0, 0.0]);
        // One layer always shows.
        let one = StackGlint::new(&rules, &[1], &SWORD, &global("method=cycle"));
        assert_eq!(one.intensity(0, 0.7), 1.0);
    }

    #[test]
    fn the_first_global_file_wins() {
        let root = std::env::temp_dir().join(format!("horizon-cit-glint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(load_global(&root), Global::default());
        let write = |path: &str, text: &str| {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write("assets/minecraft/optifine/cit.properties", "method=layered");
        write("assets/minecraft/mcpatcher/cit.properties", "method=cycle");
        assert_eq!(load_global(&root).method, Method::Layered);
        write("assets/minecraft/citresewn/cit.properties", "cap=1");
        let g = load_global(&root);
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!((g.method, g.cap), (Method::Average, Some(1)));
    }
}
