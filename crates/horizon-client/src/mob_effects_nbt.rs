//! `LivingEntity`'s saved `active_effects` list (`MobEffectInstance.CODEC`).
//!
//! `ValueInput.read("active_effects", MobEffectInstance.CODEC.listOf())`
//! decodes the whole list or none of it: one entry whose id, duration or
//! detail fails takes the list down to empty. The `readAdditionalSaveData`
//! clears the effect map first, so the default is no effects at all.
use crate::{effects::EffectInstance, mobs::Mob, save::number};
use horizon_world::nbt::{Compound, Tag};

/// One saved `MobEffectInstance` (`Details.CODEC`, `hidden_effect` included).
fn decode(tag: &Tag) -> Option<EffectInstance> {
    let def = crate::save::text(tag.get("id")).and_then(|id| crate::brewing::effect(&id))?;
    let id = crate::brewing::namespaced(def.id);
    let duration = number(tag.get("duration")).unwrap_or(0) as i32;
    // `MobEffectInstance.Details`: Codec.BYTE uses boxed Number.byteValue,
    // then UNSIGNED_BYTE interprets the resulting byte as 0..255.
    let amplifier = tag
        .get("amplifier")
        .and_then(horizon_world::datafix::dynamic::as_number)
        .map_or(0, |n| i32::from(n.byte_value() as u8));
    // Details.CODEC: `show_particles` defaults to true, `ambient` to false.
    let visible = number(tag.get("show_particles")).is_none_or(|v| v != 0);
    let ambient = number(tag.get("ambient")).is_some_and(|v| v != 0);
    let show_icon = number(tag.get("show_icon")).map_or(visible, |v| v != 0);
    let hidden = match tag.get("hidden_effect") {
        Some(hidden) => Some(Box::new(decode(hidden)?)),
        None => None,
    };
    Some(EffectInstance {
        id,
        duration,
        amplifier,
        ambient,
        visible,
        show_icon,
        hidden,
    })
}

/// `LivingEntity.readAdditionalSaveData`'s `active_effects`: apply the list
/// only when every entry decodes, as `CODEC.listOf()` does.
pub fn apply_saved(m: &mut Mob, tag: &Tag) {
    let Some(Tag::List(effects)) = tag.get("active_effects") else {
        return;
    };
    // The read clears the map before it adds the decoded list.
    m.extra.effects.clear();
    m.extra.hidden_effects.clear();
    m.extra.ambient_effects.clear();
    m.extra.effect_fallbacks.clear();
    let Some(decoded) = effects.iter().map(decode).collect::<Option<Vec<_>>>() else {
        return;
    };
    for e in decoded {
        let effect = e.id;
        m.extra.effects.retain(|x| x.0 != effect);
        m.extra.effects.push((effect, e.duration, e.amplifier));
        m.extra.hidden_effects.retain(|h| *h != effect);
        if !e.visible {
            m.extra.hidden_effects.push(effect);
        }
        m.extra.ambient_effects.retain(|h| *h != effect);
        if e.ambient {
            m.extra.ambient_effects.push(effect);
        }
        m.set_effect_fallback(effect, e.hidden);
    }
    if m.extra
        .effects
        .iter()
        .any(|e| e.0 == "minecraft:health_boost")
    {
        crate::mobs::finalize::health_boost_changed(m);
    }
}

/// `LivingEntity.addAdditionalSaveData`: `active_effects` only when there are
/// any (`MobEffectInstance.CODEC`), including modeled weaker instances.
pub fn write_saved(m: &Mob, map: &mut Compound) {
    if m.extra.effects.is_empty() {
        map.remove(&horizon_world::nbt::JavaString::from("active_effects"));
    } else {
        let effects = (0..m.extra.effects.len()).map(|i| effect_tag(&m.effect_instance(i), true));
        map.insert("active_effects".into(), Tag::List(effects.collect()));
    }
}

fn effect_tag(e: &EffectInstance, root: bool) -> Tag {
    let mut fields = Vec::new();
    if root {
        fields.push(("id", Tag::String(crate::brewing::namespaced(e.id).into())));
    }
    if e.amplifier != 0 {
        fields.push((
            "amplifier",
            Tag::Byte(e.amplifier.clamp(0, 255) as u8 as i8),
        ));
    }
    fields.push(("duration", Tag::Int(e.duration)));
    if e.ambient {
        fields.push(("ambient", Tag::Byte(1)));
    }
    if !e.visible {
        fields.push(("show_particles", Tag::Byte(0)));
    }
    fields.push(("show_icon", Tag::Byte(i8::from(e.show_icon))));
    if let Some(hidden) = e.hidden.as_deref() {
        fields.push(("hidden_effect", effect_tag(hidden, false)));
    }
    Tag::compound(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_bad_entry_takes_the_whole_list_down() {
        let tag = Tag::compound([(
            "active_effects",
            Tag::List(vec![
                Tag::compound([
                    ("id", Tag::String("minecraft:speed".into())),
                    ("duration", Tag::Int(40)),
                ]),
                Tag::compound([
                    ("id", Tag::String("minecraft:not_an_effect".into())),
                    ("duration", Tag::Int(40)),
                ]),
            ]),
        )]);
        let mut kind = crate::mobs::sensor_scan_tests::kind();
        let k = std::sync::Arc::make_mut(&mut kind);
        k.id = "minecraft:pig".into();
        k.species = crate::mobs::Species::Pig;
        let mut m = Mob::new(1, kind, horizon_core::geometry::Vec3::ZERO, 0.0);
        apply_saved(&mut m, &tag);
        assert!(m.extra.effects.is_empty(), "the list failed as a whole");
        let good = Tag::compound([(
            "active_effects",
            Tag::List(vec![Tag::compound([
                ("id", Tag::String("minecraft:speed".into())),
                ("duration", Tag::Int(40)),
            ])]),
        )]);
        apply_saved(&mut m, &good);
        assert_eq!(m.extra.effects.len(), 1);
    }
}
