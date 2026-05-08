use std::{
    io::{Read, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
};

use directories::ProjectDirs;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::Digest;

use cap_std::fs::{OpenOptionsExt as _, PermissionsExt as _};

use anyhow::Context as _;
use url::Url;

struct HashReader<'hash, R> {
    reader: R,
    hasher: &'hash mut dyn sha2::digest::Update,
}

impl<'hash, R> HashReader<'hash, R> {
    fn new(reader: R, hasher: &'hash mut dyn sha2::digest::Update) -> Self {
        Self { reader, hasher }
    }
}

impl<R: Read> Read for HashReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(buf)?;
        self.hasher.update(&buf[..count]);
        Ok(count)
    }
}

#[derive(Clone, Copy)]
struct FileLocation<'a> {
    base: &'a cap_std::fs::Dir,
    path: &'a Path,
}

impl<'a> FileLocation<'a> {
    fn new(base: &'a cap_std::fs::Dir, path: &'a Path) -> Self {
        Self { base, path }
    }
    fn open_with(&self, options: &cap_std::fs::OpenOptions) -> std::io::Result<cap_std::fs::File> {
        self.base.open_with(self.path, options)
    }
    fn exists(&self) -> bool {
        self.base.exists(self.path)
    }
    fn remove(&self) -> std::io::Result<()> {
        self.base.remove_file(self.path)
    }
    fn open(&self) -> std::io::Result<cap_std::fs::File> {
        self.open_with(cap_std::fs::OpenOptions::new().read(true))
    }
    fn create_new(&self) -> std::io::Result<cap_std::fs::File> {
        self.open_with(
            cap_std::fs::OpenOptions::new()
                .create_new(true)
                .append(true),
        )
    }
}

fn fetch_file(
    source: &str,
    target: FileLocation<'_>,
    checksum: &str,
    progress: &ProgressBar,
) -> anyhow::Result<()> {
    let url = Url::parse(source)?;
    let Some(filename) = url.path().split('/').next_back() else {
        anyhow::bail!("Could not extract filename from {source}")
    };

    let checksum_bytes = hex::decode(checksum)?;
    if target.exists() {
        let mut hasher = sha2::Sha256::default();
        std::io::copy(
            &mut HashReader::new(target.open()?, &mut hasher),
            &mut std::io::sink(),
        )?;
        if hasher.finalize().to_vec() == checksum_bytes {
            return Ok(());
        }
        target.remove()?;
    }

    progress.set_style(
        indicatif::ProgressStyle::with_template(
            "{bar:40.cyan/blue} ({bytes:>7}/{total_bytes:7}) {msg}",
        )
        .unwrap(),
    );
    progress.set_message(format!("Downloading {filename}"));

    let mut hasher = sha2::Sha256::default();
    let response = reqwest::blocking::get(source)?;
    match response.content_length() {
        Some(length) => progress.set_length(length),
        None => progress.unset_length(),
    }
    std::io::copy(
        &mut progress.wrap_read(HashReader::new(response, &mut hasher)),
        &mut target
            .create_new()
            .context("Creating archive destination")?,
    )?;
    progress.finish();
    anyhow::ensure!(
        hasher.finalize().to_vec() == checksum_bytes,
        "Downloaded file does not match checksum. URL: {source},  checksum: {checksum}"
    );
    Ok(())
}

fn unpack_file(
    source: &Path,
    target: &Path,
    checksum: &str,
    progress: &ProgressBar,
) -> anyhow::Result<()> {
    let safe_filename = source
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("no filename"))?
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("not unicode safe name: {}", source.display()))?;
    let Some(file_root) = safe_filename.strip_suffix(".tar.xz") else {
        anyhow::bail!("File does not end with .tar.xz")
    };
    let checksum_filename = format!("{file_root}.checksum");

    let checksum_path = target.with_file_name(checksum_filename);

    if target.exists() {
        match std::fs::read(&checksum_path) {
            Ok(read_checksum) => {
                if read_checksum == checksum.as_bytes() {
                    return Ok(());
                }
                std::fs::remove_file(&checksum_path)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        std::fs::remove_dir_all(target)?;
    }
    std::fs::create_dir_all(target)?;
    anyhow::ensure!(target.is_dir());

    let file = std::fs::File::open(source)?;
    progress.set_length(file.metadata()?.len());
    progress.set_style(
        ProgressStyle::with_template("{bar:40.cyan/blue} ({bytes:>7}/{total_bytes:7}) {msg}")
            .unwrap(),
    );
    progress.set_message("Unpacking");

    let safe_dir = cap_std::fs::Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
    let decoder = xz2::read::XzDecoder::new(progress.wrap_read(std::fs::File::open(source)?));
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        progress.set_message(format!("Unpacking {}...", path.display()));
        match entry.header().entry_type() {
            tar::EntryType::Regular => {
                let mut options = cap_std::fs::OpenOptions::new();
                options
                    .create_new(true)
                    .write(true)
                    .append(true)
                    .mode(entry.header().mode()? & 0o700);
                let mut new_file = safe_dir.open_with(&path, &options).context(format!(
                    "Source: {}, Root: {}, Path: {}, options: {options:?}",
                    source.display(),
                    target.display(),
                    path.display(),
                ))?;

                std::io::copy(&mut entry, &mut new_file)?;
            }
            tar::EntryType::Directory => {
                // The order should be such that
                safe_dir.create_dir(&path)?;
                safe_dir.set_permissions(
                    &path,
                    cap_std::fs::Permissions::from_mode(entry.header().mode()? & 0o700),
                )?;
            }
            tar::EntryType::Symlink => {
                let Some(dest_path) = entry.header().link_name()? else {
                    anyhow::bail!("Symlink entry w/o link name: {}", path.display())
                };
                safe_dir.symlink(&dest_path, &path)?;
            }
            e => {
                panic!("Unsupported entry type: {e:?}");
            }
        }
    }

    std::fs::write(&checksum_path, checksum)?;

    Ok(())
}

#[derive(Debug)]
struct CacheDir {
    cache_path: PathBuf,
    cache_dir: cap_std::fs::Dir,
}

impl CacheDir {
    fn new(cache_path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let cache_path = cache_path.into();
        if !cache_path.exists() {
            std::fs::create_dir_all(&cache_path)?;
        }
        if !cache_path.is_dir() {
            return Err(std::io::Error::other(
                "Cache directory exists, but is not a directory.",
            ));
        }
        let cache_dir =
            cap_std::fs::Dir::open_ambient_dir(&cache_path, cap_std::ambient_authority())?;
        Ok(Self {
            cache_path,
            cache_dir,
        })
    }

    fn fetch_file(
        &self,
        url: &str,
        path: &Path,
        checksum: &str,
        progress: &ProgressBar,
    ) -> anyhow::Result<PathBuf> {
        let target = FileLocation::new(&self.cache_dir, path);
        fetch_file(url, target, checksum, progress)?;
        Ok(self.cache_path.join(path))
    }
}

fn download_archive(
    cache_dir: &CacheDir,
    manifest_path: impl AsRef<Path>,
    multi_progress: &indicatif::MultiProgress,
    url: &str,
    checksum: &str,
    cache_file: impl AsRef<Path>,
    unpack_dir: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let archive_path = cache_dir.fetch_file(
        url,
        cache_file.as_ref(),
        checksum,
        &multi_progress.add(ProgressBar::no_length()),
    )?;
    let dest_dir = manifest_path.as_ref().join(unpack_dir);
    unpack_file(
        &archive_path,
        &dest_dir,
        checksum,
        &multi_progress.add(ProgressBar::no_length()),
    )?;
    Ok(())
}

fn ensure_remove_file(path: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::remove_file(path).or_else(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e)
        }
    })
}

pub(super) fn run_setup(base_dirs: &ProjectDirs, workspace_path: &Path) -> anyhow::Result<()> {
    let cache_dir = CacheDir::new(base_dirs.cache_dir())?;
    // Set up some basics.
    let mut term = console::Term::buffered_stderr();
    let multi_progress = indicatif::MultiProgress::with_draw_target(
        indicatif::ProgressDrawTarget::term(term.clone(), 60),
    );
    term.write_fmt(format_args!("Running setup\n"))?;
    term.flush()?;
    download_archive(
        &cache_dir,
        workspace_path,
        &multi_progress,
        "https://github.com/Gcenx/macOS_Wine_builds/releases/download/11.0_1/wine-stable-11.0_1-osx64.tar.xz",
        "b50dc50ec7f41d58b115a6b685d4d1315ba3c797bd3aa0f49213f2703cb82388",
        "wine-bin.tar.xz",
        ".tools/dist/wine",
    )?;
    // Link the wine binary into the tools prefix
    std::fs::create_dir_all(workspace_path.join(".tools/bin"))?;
    let wine_bin_path = workspace_path.join(".tools/bin/wine");
    ensure_remove_file(&wine_bin_path)?;
    std::os::unix::fs::symlink(
        "../dist/wine/Wine Stable.app/Contents/Resources/wine/bin/wine",
        wine_bin_path,
    )?;

    Ok(())
}

pub(super) fn run_env(
    workspace_path: &Path,
    cmd: &str,
    args: &[String],
) -> anyhow::Result<ExitCode> {
    let addl_path = [workspace_path.join(".tools/bin")];

    let path_env = std::env::var_os("PATH").unwrap_or_default();

    let new_path_elems = addl_path
        .into_iter()
        .chain(std::env::split_paths(&path_env));
    let new_path_env = std::env::join_paths(new_path_elems)?;

    let mut signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGCHLD,
    ])?;
    // Start the child
    let mut child = std::process::Command::new(cmd)
        .args(args)
        .env("PATH", new_path_env)
        .spawn()?;

    // Wait for either a SIGTERM, or a signal that our child process
    // has exited.
    for signal in &mut signals {
        match signal {
            signal_hook::consts::SIGTERM => {
                child.kill()?;
            }
            signal_hook::consts::SIGCHLD => {
                break;
            }
            _ => unreachable!(),
        }
    }

    let status = child.wait()?;

    if !status.success() {
        return Ok(status
            .code()
            .and_then(|c| u8::try_from(c).ok())
            .map_or(ExitCode::FAILURE, ExitCode::from));
    }

    Ok(ExitCode::SUCCESS)
}
