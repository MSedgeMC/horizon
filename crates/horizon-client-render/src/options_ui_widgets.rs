//! The option screens' widgets and shared labels: a 20-pixel button row, the
//! same widget turned inactive, and the Chat Delay message.
use super::*;

pub(crate) fn button(x: f32, y: f32, w: f32, label: Text, act: Act) -> Widget {
    Widget {
        x,
        y,
        w,
        h: 20.0,
        kind: WKind::Button {
            label,
            active: true,
            act,
        },
        list: false,
        row: 0,
    }
}
pub(crate) fn inactive(mut w: Widget) -> Widget {
    if let WKind::Button { active, .. } = &mut w.kind {
        *active = false;
    }
    w
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
