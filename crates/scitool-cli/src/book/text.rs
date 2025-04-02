use std::str::FromStr;

pub enum FontControl {
    Default,
    Italics,
    SuperLarge,
    Lowercase,
    Title,
    BoldLike,
    Unknown,
}

pub enum ColorControl {
    Default,
    Red,
    Yellow,
    White,
    Green,
    Cyan,
    Unknown,
}

pub enum Control {
    Font(FontControl),
    Color(#[expect(dead_code)] ColorControl),
}

pub enum MessageSegment {
    Text(String),
    Control(Control),
}

fn split_first_char(text: &str) -> Option<(char, &str)> {
    let mut chars = text.chars();
    let first = chars.next()?;
    Some((first, chars.as_str()))
}

fn parse_control(ctrl: char, value: Option<u32>) -> anyhow::Result<Control> {
    Ok(match ctrl {
        'f' => Control::Font(match value {
            None => FontControl::Default,
            Some(1) => FontControl::Unknown,
            Some(2) => FontControl::Italics,
            Some(3) => FontControl::SuperLarge,
            Some(4) => FontControl::Lowercase,
            Some(5) => FontControl::Title,
            Some(6) => FontControl::Unknown,
            Some(8) => FontControl::BoldLike,
            Some(n) => anyhow::bail!("Unexpected font control value: {}", n),
        }),

        // Color control
        'c' => Control::Color(match value {
            None => ColorControl::Default,
            Some(1) => ColorControl::Red,
            Some(2) => ColorControl::Yellow,
            Some(3) => ColorControl::White,
            Some(4) => ColorControl::Green,
            Some(5) => ColorControl::Cyan,
            Some(6) => ColorControl::Unknown,
            Some(n) => anyhow::bail!("Unexpected color control value: {}", n),
        }),
        c => anyhow::bail!("Unexpected control value: ({}, {:?})", c, value),
    })
}

fn parse_message_text(mut text: &str) -> anyhow::Result<Vec<MessageSegment>> {
    let mut segments = Vec::new();
    loop {
        let (next_text, control_start_rest) = match text.split_once('|') {
            Some((first, second)) => (first, Some(second)),
            None => (text, None),
        };

        if !next_text.is_empty() {
            segments.push(MessageSegment::Text(next_text.to_string()));
        }

        let Some(rest) = control_start_rest else {
            break;
        };
        let (control, rest) = split_first_char(rest).unwrap();
        let (value, rest) = rest.split_once('|').unwrap();
        segments.push(MessageSegment::Control(parse_control(
            control,
            if value.is_empty() {
                None
            } else {
                Some(value.parse().unwrap())
            },
        )?));
        text = rest;
    }

    Ok(segments)
}

pub struct MessageText {
    segments: Vec<MessageSegment>,
}

impl MessageText {
    pub fn segments(&self) -> &[MessageSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl FromStr for MessageText {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let segments = parse_message_text(text)?;
        Ok(MessageText { segments })
    }
}
