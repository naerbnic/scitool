use std::{collections::BTreeSet, path::Path};

use scidev::resources::{ResourceSet, ResourceType, types::msg::parse_message_resource};

pub fn print_talkers(game_dir: &Path, mut output: impl std::io::Write) -> anyhow::Result<()> {
    let resource_set = ResourceSet::from_root_dir(game_dir)?;
    let mut talkers = BTreeSet::new();
    for res in resource_set.resources_of_type(ResourceType::Message) {
        let msg_resources = parse_message_resource(&res.data().open_mem(..)?)?;
        for (_, record) in msg_resources.messages() {
            talkers.insert(record.talker());
        }
    }
    write!(output, "Talkers:")?;
    write!(
        output,
        "  {}",
        talkers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    Ok(())
}
