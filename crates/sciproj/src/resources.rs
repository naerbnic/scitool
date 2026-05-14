use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use itertools::Itertools;
use scidev::{
    ids::{
        LineId,
        raw::{RawConditionId, RawNounId, RawRoomId, RawSequenceId, RawVerbId},
    },
    resources::types::msg::MessageId,
};
use serde::{Deserialize, Serialize};

use crate::imp::futures::{prelude::*, stream::FuturesUnordered};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AudioClip {
    pub start_us: Option<u64>,
    pub end_us: Option<u64>,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sample {
    pub room: u16,
    pub message_id: MessageId,
    pub clip: AudioClip,
}

#[derive(Serialize, Deserialize, Debug)]
struct SampleSet(Vec<Sample>);

pub fn legacy_load_dir(path: &Path) -> anyhow::Result<BTreeMap<LineId, AudioClip>> {
    let samples_file = path.join("samples.json");
    let samples_file_contents = std::fs::read(&samples_file)?;
    let mut sample_set: SampleSet =
        serde_json::from_reader(std::io::Cursor::new(samples_file_contents))?;
    let mut clip_map = BTreeMap::new();
    for sample in &mut sample_set.0 {
        let Sample {
            room,
            message_id,
            clip,
        } = sample;
        let line_id = LineId::from_parts(
            RawRoomId::new(*room),
            RawNounId::new(message_id.noun()),
            RawVerbId::new(message_id.verb()),
            RawConditionId::new(message_id.condition()),
            RawSequenceId::new(message_id.sequence()),
        );
        let relative_path = std::mem::take(&mut clip.path);
        clip.path = path.join(relative_path);
        clip_map.insert(line_id, clip.clone());
    }
    Ok(clip_map)
}

pub struct SampleDir {
    base_path: PathBuf,
    samples: SampleSet,
}

impl SampleDir {
    pub fn load_dir(path: &Path) -> anyhow::Result<Self> {
        let samples_file = path.join("samples.json");
        let samples_file_contents = std::fs::read(&samples_file)?;
        let sample_set: SampleSet =
            serde_json::from_reader(std::io::Cursor::new(samples_file_contents))?;
        Ok(Self {
            base_path: path.to_path_buf(),
            samples: sample_set,
        })
    }

    pub async fn save_to_scannable_dir(&self, path: &Path) -> anyhow::Result<()> {
        // Check that all files contain a single message ID.
        let path_list = self
            .samples
            .0
            .iter()
            .map(|sample| {
                let clip = &sample.clip;
                anyhow::ensure!(clip.start_us.is_none_or(|off| off == 0));
                anyhow::ensure!(clip.end_us.is_none());
                Ok(&sample.clip.path)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let multi_path_counts = path_list
            .into_iter()
            .map(|path| (path, 1))
            .into_grouping_map()
            .sum()
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .collect::<Vec<_>>();
        if !multi_path_counts.is_empty() {
            return Err(anyhow::anyhow!(
                "The following paths have multiple message IDs: {multi_path_counts:?}"
            ));
        }

        // Go through all of the clips, and copy the files with the line ID
        // as the file name.
        let mut copy_operations = self
            .samples
            .0
            .iter()
            .map(async |sample| {
                let line_id = LineId::from_parts(
                    RawRoomId::new(sample.room),
                    RawNounId::new(sample.message_id.noun()),
                    RawVerbId::new(sample.message_id.verb()),
                    RawConditionId::new(sample.message_id.condition()),
                    RawSequenceId::new(sample.message_id.sequence()),
                );
                let clip = &sample.clip;
                let current_path = &clip.path;
                let mut file_name: OsString = line_id.to_string().into();
                if let Some(ext) = current_path.extension() {
                    file_name.push(".");
                    file_name.push(ext);
                }
                let mut new_path = current_path.clone();
                new_path.set_file_name(file_name);
                let source_path = self.base_path.join(current_path);
                let target_path = path.join(new_path);
                // Create the target directory if it doesn't exist.
                let target_dir = target_path.parent().unwrap();
                tokio::fs::create_dir_all(target_dir).await?;
                // Copy the file to the new location.
                tokio::fs::create_dir_all(target_dir).await?;
                tokio::fs::copy(source_path, target_path).await?;
                Ok::<_, anyhow::Error>(())
            })
            .collect::<FuturesUnordered<_>>();

        while let Some(result) = copy_operations.next().await {
            result?;
        }
        Ok(())
    }
}
