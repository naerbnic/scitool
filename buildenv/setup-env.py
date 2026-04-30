#!/usr/bin/env python3

from typing import final
from tempfile import TemporaryDirectory
from typing import Self
from typing import Any
from asyncio import subprocess
import functools
from collections.abc import Buffer
from collections import abc
from typing import Protocol
from typing import Callable
import contextlib
from dataclasses import dataclass
import hashlib
import io
import os
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import urllib.request
import urllib.parse


class Error(Exception):
    pass


class CommandNotFound(Error):
    pass


class FailedChecksum(Error):
    pass


@dataclass(frozen=True)
class RequiredBinaries:
    brew: Path
    git: Path
    tar: Path


type ProgressHook = Callable[[str, int, int, bool], None]

_SUFFIXES = ["", "K", "M", "G", "T"]


def HumanRepr(value: int, base: int = 1024) -> str:
    if value == 0:
        return "0"
    for suffix in _SUFFIXES[:-1]:
        if value < base:
            return f"{value:.3g}{suffix}"
        value //= base
    return f"{value:.3g}{_SUFFIXES[-1]}"


def SimpleProgress(file: str, read: int, size: int | None, done: bool) -> None:
    line_start_str = ""
    line_end_str = ""
    if sys.stdout.isatty():
        line_start_str = "\x1b[K"
        line_end_str = "\x1b[0G"
    pct: str = ""
    if size is not None:
        pct = f"{read / size:.2%}% "
    sys.stdout.write(
        f"{line_start_str}Downloading {file}: {pct}({HumanRepr(read)}B/{HumanRepr(size) + 'B' or '?'}){line_end_str}"
    )
    if done:
        sys.stdout.write("\n")
    sys.stdout.flush()


class HashChecker(Protocol):
    @abc.abstractmethod
    def update(self, data: Buffer): ...

    @abc.abstractmethod
    def checksum(self) -> str: ...

    def checksum_matches(self, checksum: str) -> bool:
        return self.checksum() == checksum

    @final
    def check(self, checksum: str):
        if not self.checksum_matches(checksum):
            raise FailedChecksum(f"Checksum mismatch: {self.checksum()} != {checksum}")

    @final
    def update_file(self, file: io.Reader):
        num_bytes = 8 * 1024
        while chunk := file.read(num_bytes):
            self.update(chunk)

    @final
    def check_file(self, file: io.Reader, checksum: str):
        self.update_file(file)
        self.check(checksum)


class NullHashChecker(HashChecker):
    def update(self, data: Buffer):
        pass

    def checksum(self) -> str:
        return ""

    def checksum_matches(self, _checksum: str) -> bool:
        return True


class HashLibChecker(HashChecker):
    def __init__(self, alg: str):
        self._hasher = hashlib.new(alg)

    def update(self, data: Buffer):
        self._hasher.update(data)

    def checksum(self) -> str:
        return f"{self._hasher.name}:{self._hasher.digest().hex()}"


def get_url_filename(url: str) -> str:
    return urllib.parse.urlsplit(url).path.rsplit("/", 1)[-1]


def fetch_url(
    url: str,
    *,
    dest: Path,
    checksum: str | None = None,
    progress: ProgressHook | None = None,
) -> tuple[Path, str]:
    if progress is None:

        def NullProgress(file, read, size, done):
            pass

        progress = NullProgress

    checksum_alg = "sha256" if checksum is None else checksum.split(":", 1)[0]

    filename = get_url_filename(url)

    if dest.exists():
        hash_checker = HashLibChecker(checksum_alg)
        with dest.open("rb") as f:
            hash_checker.update_file(f)
        if checksum is not None:
            if hash_checker.checksum_matches(checksum):
                progress(filename, dest.stat().st_size, dest.stat().st_size, True)
                return (dest, checksum)

    hash_checker = HashLibChecker(checksum_alg)

    with urllib.request.urlopen(url) as resp:
        size = None
        if "Content-Length" in resp.headers:
            size = int(resp.headers["Content-Length"])
        read = 0
        progress(filename, read, size, False)
        with dest.open("wb") as f:
            buf_size = 8 * 1024
            while b := resp.read(buf_size):
                read += len(b)
                progress(filename, read, size, False)
                hash_checker.update(b)
                f.write(b)
        progress(filename, read, size, True)
    if checksum:
        hash_checker.check(checksum)
    return (dest, hash_checker.checksum())


@contextlib.contextmanager
def make_temp_path(path: Path | None = None, base: Path | None = None) -> Path:
    if path is None:
        with tempfile.TemporaryDirectory(dir=base) as path_str:
            yield Path(path_str)
    else:
        try:
            path.mkdir(parents=True)
            yield path
        finally:
            path.rmdir()


@dataclass(frozen=True)
class BaseDirectories:
    root: Path

    cache: Path
    runtime: Path


def get_base_dirs() -> BaseDirectories:
    # Create short unique hash of current script path
    script_path = Path(__file__).resolve()
    script_hash = hashlib.sha256(script_path.as_posix().encode()).hexdigest()[:8]

    root = script_path.parent.parent
    cache = os.environ.get("XDG_CACHE_DIR", default="")
    if not cache:
        assert "HOME" in os.environ
        cache = f"{os.environ['HOME']}/.cache"
    cache = Path(cache) / f"setup-env-{script_hash}"
    cache.mkdir(mode=0o700, parents=True, exist_ok=True)

    runtime = os.environ.get("XDG_RUNTIME_DIR", default="")
    if not runtime:
        runtime = "/tmp"
    runtime = Path(runtime)
    runtime.mkdir(mode=0o700, parents=True, exist_ok=True)

    return BaseDirectories(root=root, cache=cache, runtime=runtime)


class EnvBuilder:
    _path_env: list[str]
    _which: dict[str, Path]
    _tmp: TemporaryDirectory | None

    def __init__(self):
        self._path_env = os.environ.get("PATH").split(":")
        self._which = {}
        self._tmp = None

    @functools.cached_property
    def dirs(self) -> BaseDirectories:
        return get_base_dirs()

    @property
    def tmpdir(self) -> Path:
        if self._tmp is None:
            self._tmp = tempfile.TemporaryDirectory(dir=self.dirs.runtime)
        return Path(self._tmp.name)

    @functools.cache
    def which(self, cmd: str) -> Path:
        cmd_path = shutil.which(cmd, path=":".join(self._path_env))
        if cmd_path is None:
            raise CommandNotFound(f"Command '{cmd}' not found in PATH")
        return Path(cmd_path)

    def fetch_archive(
        self,
        url: str,
        dest: Path,
        *,
        checksum: str | None = None,
    ) -> Path:
        filename = Path(get_url_filename(url))
        cache_name = "".join([dest.name, *filename.suffixes])
        archive_path, checksum = fetch_url(
            url,
            dest=self.dirs.cache / cache_name,
            checksum=checksum,
            progress=SimpleProgress,
        )

        checksum_file = dest.with_suffix(".checksum")
        if checksum_file.exists():
            with open(checksum_file, "r") as f:
                existing = f.read().strip()
            if checksum == existing:
                # The archive was expanded with the same checksum. Skip it.
                return dest

        shutil.rmtree(dest)
        dest.mkdir(mode=0o700, parents=True)

        self.call(
            "tar",
            "-xzf",
            archive_path,
            "--cd",
            dest,
            "--strip-components=1",
        )

        # Write checksum file to mark that we've expanded this archive

        with open(checksum_file, "w") as f:
            f.write(checksum)

        return dest

    def get_env(
        self,
        env: dict[str, str | bytes | Path] | None = None,
        *,
        addl_env: dict[str, str | bytes | Path] | None = None,
    ):
        new_env = dict(os.environ if env is None else env)
        if addl_env:
            new_env.update(addl_env)
        new_env.update(PATH=":".join(self._path_env))
        return new_env

    def run_cmd(
        self,
        cmd: str | Path,
        args: list[str],
        *,
        addl_env: dict[str, str | bytes | Path] | None = None,
        **kwargs,
    ) -> subprocess.CompletedProcess[Any]:
        if isinstance(cmd, str):
            cmd = Path(cmd)
        if not cmd.is_absolute() and len(cmd.parts) == 1:
            cmd = self.which(str(cmd))

        return subprocess.run(
            [cmd, *args],
            cwd=kwargs.pop("cwd", self.dirs.root),
            env=self.get_env(kwargs.pop("env", None), addl_env=addl_env),
            **kwargs,
        )

    def call(
        self,
        cmd: str | Path,
        *args: list[str | bytes | Path],
        **kwargs,
    ):
        self.run_cmd(cmd, args, check=True, **kwargs)

    def get_cmd_out(self, cmd: str | Path, *args, **kwargs) -> None:
        res = self.run_cmd(
            cmd,
            list(args),
            check=True,
            capture_output=True,
            text=True,
            **kwargs,
        )

        return res.stdout.strip()

    def get_brew_prefix(self, formula: str) -> Path:
        return Path(self.get_cmd_out("brew", "--prefix", formula))

    def prepend_path(self, path: str | Path):
        self._path_env.insert(0, str(path))

    def brew_install(self, *formulae: list[str]) -> None:
        self.call(
            "brew",
            "install",
            *formulae,
        )

        for f in formulae:
            self.prepend_path(f"{self.get_brew_prefix(f)}/bin")

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *args):
        if self._tmp is not None:
            self._tmp.cleanup()


def main(argv):
    del argv

    with EnvBuilder() as builder:
        tools_dir = builder.dirs.root / ".tools"

        tools_dir.mkdir(mode=0o700, parents=True, exist_ok=True)

        llvm_mingw_root = builder.fetch_archive(
            "https://github.com/mstorsjo/llvm-mingw/releases/download/20260421/llvm-mingw-20260421-ucrt-macos-universal.tar.xz",
            dest=tools_dir / "src" / "llvm-mingw",
            checksum="sha256:bd85a3975723815cef28dbbd2ca2cb0c926f6b348a12a0453f39f7af273cb3f7",
        )

        wine_root = builder.fetch_archive(
            "https://dl.winehq.org/wine/source/11.x/wine-11.7.tar.xz",
            dest=tools_dir / "src" / "wine",
            checksum="sha256:b01ab21c79fede6c7bd531d469d99afd9dcdf53eb29af88adac6a332eb435f9f",
        )

        builder.prepend_path(f"{llvm_mingw_root}/bin")

        builder.brew_install(
            "llvm",
            "bison",
        )

        builder.call(
            wine_root / "configure",
            f"--prefix={tools_dir}",
            "--enable-win64",
            "--with-mingw",
            "--enable-archs=x86_64,aarch64",
            cwd=wine_root,
        )

        num_cpus = builder.get_cmd_out("sysctl", "-n", "hw.activecpu")
        builder.call(
            "make",
            f"-j{num_cpus}",
            cwd=wine_root,
        )

        builder.call(
            "make",
            "install",
            cwd=wine_root,
        )


if __name__ == "__main__":
    import sys

    main(sys.argv)
