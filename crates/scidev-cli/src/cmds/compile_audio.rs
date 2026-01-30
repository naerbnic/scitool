use std::path::{Path, PathBuf};

use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};
use sciproj::{
    file::AudioSampleScan, path::LookupPath, resources::SampleDir, tools::ffmpeg::FfmpegTool,
};

#[derive(Copy, Clone, Debug)]
pub(crate) enum ScanType {
    Legacy,
    Scannable,
}

pub(crate) fn compile_audio(
    scan_type: ScanType,
    sample_dir: &Path,
    output_dir: &Path,
) -> anyhow::Result<()> {
    let system_path = LookupPath::from_env();
    log::info!("System PATH: {:?}", system_path.find_binary("ffmpeg"));
    let ffmpeg_tool = FfmpegTool::from_path(
        system_path
            .find_binary("ffmpeg")
            .expect("ffmpeg not found in PATH")
            .to_path_buf(),
        system_path
            .find_binary("ffprobe")
            .expect("ffprobe not found in PATH")
            .to_path_buf(),
    );
    let sample_dir = match scan_type {
        ScanType::Legacy => SampleDir::load_dir(sample_dir)?,
        ScanType::Scannable => {
            let scan = AudioSampleScan::read_from_dir(sample_dir)?;
            anyhow::ensure!(
                !scan.has_duplicates(),
                "Duplicate files found in scan directory",
            );
            SampleDir::from_sample_scan(&scan)?
        }
    };
    let resources = sample_dir.to_audio_resources(&ffmpeg_tool)?;

    let aud_file_task = std::thread::spawn({
        let resources = resources.clone();
        let output_dir = output_dir.to_path_buf();
        move || {
            let resource_aud_file = std::fs::File::create(output_dir.join("resource.aud"))?;
            let mut reader = resources.audio_volume().open_reader(..)?;
            std::io::copy(
                &mut reader,
                &mut std::io::BufWriter::new(&resource_aud_file),
            )?;
            Ok::<_, anyhow::Error>(())
        }
    });

    resources
        .map_resources()
        .par_iter()
        .map(|res| {
            let file = PathBuf::from(format!(
                "{}.{}",
                res.id().resource_num(),
                res.id().type_id().to_file_ext()
            ));
            let open_file = std::fs::File::create(output_dir.join(&file))?;
            res.write_patch(open_file)?;
            Ok::<_, anyhow::Error>(())
        })
        .collect::<anyhow::Result<()>>()?;

    aud_file_task.join().unwrap()?;

    Ok(())
}
