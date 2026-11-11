//! Option labels that say their value.
use super::*;

/// `Options.chatDelay`'s message: None at zero, else the delay in seconds to
/// one decimal, not "None" at every value.
#[test]
fn the_chat_delay_slider_says_its_delay() {
    let mut s = Settings::default();
    assert_eq!(chat_delay_label(&s).plain(), "options.chat.delay_none");
    s.file.set("chatDelay", crate::options_file::V::Double(2.0));
    assert_eq!(chat_delay_label(&s).plain(), "Chat Delay: 2.0 second(s)");
    s.file.set("chatDelay", crate::options_file::V::Double(6.0));
    assert_eq!(chat_delay_label(&s).plain(), "Chat Delay: 6.0 second(s)");
    s.file.set("chatDelay", crate::options_file::V::Double(0.0));
    assert_eq!(chat_delay_label(&s).plain(), "options.chat.delay_none");
}
