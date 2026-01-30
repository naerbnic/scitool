use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    path::{Path, PathBuf},
};

use scidev::ids::LineId;

fn is_audio_file_ext(ext: &OsStr) -> bool {
    let Some(ext) = ext.to_str() else {
        return false;
    };
    let ext = ext.to_lowercase();
    matches!(
        &*ext,
        "wav" | "mp3" | "ogg" | "flac" | "m4a" | "opus" | "aac"
    )
}

pub struct AudioSampleScan {
    base_path: PathBuf,
    entries: Vec<AudioSampleEntry>,
    line_id_map: BTreeMap<LineId, BTreeSet<usize>>,
}

impl AudioSampleScan {
    pub fn read_from_dir(base_path: &Path) -> anyhow::Result<Self> {
        let mut entries = Vec::new();
        for dir_entry in walkdir::WalkDir::new(base_path).same_file_system(true) {
            let dir_entry = dir_entry?;
            if dir_entry.file_type().is_dir() {
                continue;
            }
            let path = dir_entry.path();
            // WalkDir returns paths with a prefix of the base path. Remove the
            // prefix.
            let path = path
                .strip_prefix(base_path)
                .map_err(|_| anyhow::anyhow!("Failed to strip prefix"))?;

            if path.extension().is_some_and(is_audio_file_ext) {
                // This is an audio file.
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if let Ok(line_id) = stem.parse() {
                    // The stem is a valid line ID. Add it to our list.
                    entries.push(AudioSampleEntry::PlainFile {
                        line_id,
                        path: path.to_path_buf(),
                    });
                }
            }
        }

        let mut line_id_map = BTreeMap::new();
        for (i, entry) in entries.iter().enumerate() {
            for line_id in entry.line_ids() {
                line_id_map
                    .entry(line_id)
                    .or_insert_with(BTreeSet::new)
                    .insert(i);
            }
        }
        Ok(AudioSampleScan {
            base_path: base_path.to_path_buf(),
            entries,
            line_id_map,
        })
    }

    #[must_use]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn entries(&self) -> impl Iterator<Item = &AudioSampleEntry> + use<'_> {
        self.entries.iter()
    }

    #[must_use]
    pub fn has_duplicates(&self) -> bool {
        self.line_id_map
            .iter()
            .any(|(_, entries)| entries.len() > 1)
    }

    pub fn get_duplicates(
        &'_ self,
    ) -> impl Iterator<Item = (LineId, impl IntoIterator<Item = AudioSample<'_>>)> {
        self.line_id_map
            .iter()
            .filter(|(_, entries)| entries.len() > 1)
            .map(|(line_id, entries)| {
                let entries = entries
                    .iter()
                    .map(|&i| self.entries[i].get_sample(*line_id).unwrap());
                (*line_id, entries)
            })
    }

    pub fn get_valid_entries(&'_ self) -> impl Iterator<Item = (LineId, AudioSample<'_>)> {
        self.line_id_map
            .iter()
            .filter(|(_, entries)| entries.len() == 1)
            .map(|(line_id, entries)| {
                let entry = self.entries[*entries.iter().next().unwrap()]
                    .get_sample(*line_id)
                    .unwrap();
                (*line_id, entry)
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AudioSample<'a> {
    start: f32,
    end: Option<f32>,
    path: &'a Path,
}

impl AudioSample<'_> {
    #[must_use]
    pub fn start(&self) -> f32 {
        self.start
    }

    #[must_use]
    pub fn end(&self) -> Option<f32> {
        self.end
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.path
    }
}

pub enum AudioSampleEntry {
    PlainFile { line_id: LineId, path: PathBuf },
    // SampleSet { line_id: LineId, path: PathBuf },
}

impl AudioSampleEntry {
    #[must_use]
    pub fn line_ids(&self) -> Vec<LineId> {
        match self {
            AudioSampleEntry::PlainFile { line_id, .. } => vec![*line_id],
        }
    }

    #[must_use]
    pub fn get_sample(&'_ self, line_id: LineId) -> Option<AudioSample<'_>> {
        match self {
            AudioSampleEntry::PlainFile { line_id: id, path } => {
                if *id == line_id {
                    Some(AudioSample {
                        start: 0.0,
                        end: None,
                        path,
                    })
                } else {
                    None
                }
            }
        }
    }
}
