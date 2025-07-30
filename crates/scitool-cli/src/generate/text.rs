use scidev_book::{self as book, rich_text::RichText};

#[must_use]
pub(crate) fn make_room_title(room: &book::Room<'_>) -> RichText {
    let mut room_title_builder = RichText::builder();
    room_title_builder.add_plain_text(&room.name().unwrap_or("*NO NAME*"));
    room_title_builder.build()
}

pub(crate) fn make_conversation_title(conv: &book::Conversation<'_>) -> RichText {
    RichText::from(match (conv.verb(), conv.condition()) {
        (Some(verb), Some(condition)) => format!(
            "On {} ({})",
            verb.name(),
            condition.desc().map_or_else(
                || format!("Condition #{:?}", condition.id().condition_num()),
                ToString::to_string
            )
        ),
        (Some(verb), None) => format!("On {}", verb.name()),
        (None, Some(condition)) => format!(
            "When {}",
            condition.desc().map_or_else(
                || format!("Condition #{:?}", condition.id().condition_num()),
                ToString::to_string
            )
        ),
        (None, None) => "On Any".to_string(),
    })
}

pub(crate) fn make_noun_title(noun: &book::Noun<'_>) -> RichText {
    let mut noun_desc = noun.desc().map_or_else(
        || format!("Noun #{:?}", noun.id().noun_num()),
        ToOwned::to_owned,
    );

    if noun.is_cutscene() {
        noun_desc.push_str(" (Cutscene)");
    }
    RichText::from(noun_desc)
}
