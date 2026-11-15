//! The small option value labels (`generic`, `percent`, `on_off`, `chat_px`,
//! `chat_delay_label`) and `Options.chatDelay`'s slider mapping.
use super::*;

pub(crate) fn generic(caption: &str, value: Text) -> Text {
    Text::translate("options.generic_value", &[tr(caption), value])
}
pub(crate) fn on_off(v: bool) -> Text {
    tr(if v { "options.on" } else { "options.off" })
}
/// `CommonComponents.optionStatus(name, value)`: the composed form, so a
/// language can word the caption and the state together as one phrase. The
/// boolean-only overload is `on_off`; `Options.chatScale`, `mipmapLevels` and
/// `maxAnisotropyBit` use this one at zero.
pub(crate) fn option_status(caption: &str, on: bool) -> Text {
    Text::translate(
        if on {
            "options.on.composed"
        } else {
            "options.off.composed"
        },
        &[tr(caption)],
    )
}
pub(crate) fn chat_px(caption: &str, px: f32) -> Text {
    Text::translate(
        "options.pixel_value",
        &[tr(caption), Text::literal((px as i32).to_string())],
    )
}
pub(crate) fn percent(caption: &str, value: f32) -> Text {
    Text::translate(
        "options.percent_value",
        &[
            tr(caption),
            Text::literal(((f64::from(value) * 100.0) as i32).to_string()),
        ],
    )
}
/// `Options.chatDelay`'s message: `options.chat.delay_none` at zero, else
/// `options.chat.delay` with the delay to one decimal (`%.1f` seconds).
pub(crate) fn chat_delay_label(s: &Settings) -> Text {
    let value = s.file.double("chatDelay") as f32;
    if value <= 0.0 {
        Text::translate("options.chat.delay_none", &[])
    } else {
        Text::translate_with_fallback(
            "options.chat.delay",
            "Chat Delay: %s second(s)",
            &[Text::literal(format!("{value:.1}"))],
        )
    }
}
/// `IntRange(0, 60).toSliderValue` on the stored seconds (`xmap(v / 10.0)`):
/// the tenth-of-a-second step over the slider's 60.
pub(crate) fn kind(value: f64) -> Kind {
    Kind::Slider(((value * 10.0).floor() / 60.0).clamp(0.0, 1.0) as f32)
}
/// `IntRange(0, 60).fromSliderValue` then `xmap`'s `/ 10.0`:
/// floor(map(v, 0, 1, 0, 61)), the last step capped at 60, in tenths.
pub(crate) fn from_slider(v: f32) -> f64 {
    f64::from((v.clamp(0.0, 1.0) * 61.0).floor().min(60.0) / 10.0)
}
/// `Options.chatDelay.set`: the slider's value as the stored seconds.
pub(crate) fn set(s: &mut Settings, v: f32) {
    s.file
        .set("chatDelay", crate::options_file::V::Double(from_slider(v)));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `CommonComponents.optionStatus(name, false)`: the reachable zero-valued
    /// options, Chat Scale and Mipmaps, say `options.off.composed` with the
    /// caption, while a value keeps the generic caption-and-value label.
    #[test]
    fn a_zero_option_says_the_composed_off_label() {
        let mut s = Settings::default();
        s.chat_scale = 0.0;
        let label = Opt::ChatScale.label(&s);
        assert_eq!(label.translation_key(), Some("options.off.composed"));
        // The composed key takes the caption as its one argument.
        let source = label.component();
        assert_eq!(source["with"][0]["translate"], "options.chat.scale");
        s.mipmap_levels = 0;
        assert_eq!(
            Opt::Mipmaps.label(&s).translation_key(),
            Some("options.off.composed")
        );
        // A nonzero Chat Scale is still options.percent_value with a value.
        s.chat_scale = 0.5;
        assert_eq!(
            Opt::ChatScale.label(&s).translation_key(),
            Some("options.percent_value")
        );
    }
}
