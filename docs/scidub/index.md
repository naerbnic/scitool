# The SCIDub Tool

The `scidub` tool is used to create, manage, and build fan dub projects on the SCI engine.

## Installation

At the moment, you will have to build from source in order to run `scidub`.
In the future, I will try to provide pre-built versions of the tool.

### Build Prerequisites

Install Rust using the `rustup` tool.
You can follow the [official directions](https://rustup.rs/) to install it.
Install the latest stable version of rust, when asked.

Install Git from your platform's package manager, or from the [official site](https://git-scm.com/).

### Getting the Source

You may have already done this if you're reading this file.
If not, check out the repository in the current directory by running.

```shell
git clone https://github.com/naerbnic/scitool.git
```

This will create a directory named `scitool` in the current directory.

### Building

Move into the repository directory and run:

```shell
cargo build --release -p scidev
```

After a few minutes, the build should complete successfully.
The binary will be at `./target/release/scidev`.

## Using `scidev`

### Prerequisites

The tool itself has dependencies on a few command-line programs.

- [ffmpeg](https://ffmpeg.org/download.html)

  Used to convert audio formats to ones that can be used by ScummVM.

- [espeak](https://github.com/espeak-ng/espeak-ng) (Optional)

  Used to generate placeholder voice samples.
  Note that this is using classic TTS, not any GenAI method of generating voices.

These tools should be on your system path (i.e. you should be able to type `ffmpeg` or `espeak` from your command line, and it should run those programs).

Finally, you will likely need [ScummVM](https://www.scummvm.org/) in order to run the game once complete.

### Running `scidev`

The tool uses a command-line interface to run different operations.
You can browse the commands using:

```shell
scidev help
```

Feel free to browse around.

### Creating a Project

To start a new project, create a new empty directory, and run:

```shell
scidev init --game-data <path to the game directory>
```

This will do a few things:

- Create an initial scidub.toml file, which is the main project config
- Create a `.gitignore` file that ignores build outputs and the game data
  files.
- If you pass `--game-data`, copies the game data files into the directory,
  and creates a manifest to record the exact version of files that were imported.

From here, we can try to create an initial "book" (i.e. voice script) for the game.
Try to run:

```shell
scidev script export book -o <path to a .json file>
```

If this generates a file as an output, then the project should be working!

I recommend you use Git to track the files in this directory.
Note that you may want to set up [Git LFS](https://git-lfs.com/) to deal with audio files, if you plan to store them directly in the repository.

### Workflow

This tool is intended to be used along with a spreadsheet tool (such as Google Sheets or Microsoft Excel) to keep track of the different lines to be recorded. We expect that any audio files will eventually be somewhere under the project directory.

There are two ways to export the lines for the game: First, as above, you can generate a book JSON file.
This can be used with the webapp to post the entire game script in a way that should be acceptable to the voice actors, and to overview different parts of the script.
Note that you can customize the script further by adding a book config, but that is another sections.

The other way to export the lines is as a CSV file.
Run:

```shell
scidub script export lines -o <output path>
```

This will generate a CSV file that should be importable into Excel or Sheets.
This spreadsheet tracks the IDs of each line, their text, and the role that was marked as the performer.
A project manager can extend this data with other columns as is needed for managing the actual recording process.
You can always re-export the lines, and the `id` column will be consistent, so with some spreadsheet magic, you should be able to update any of these columns with new data as we proceed.

To create an audio patch, you need to create a CSV file with columns that have headers of `line_id`, and `clip_path`.
The line ID must be the exact same ID that was generated in the lines file.
The `clip_path` must be a relative path from the project root to the audio file.
If you have audio files with multiple clips in it, you may also provide `clip_start_ns` and `clip_end_ns` columns, which take the nanosecond offset from
the start of the file to the beginning and end of the clip respectively.
This file can also be checked into the project repostiory.

Now, you can build the project by running:

```shell
scidub build -m <mapping CSV file>
```

This will build a new game under the `build/<file name>/` directory, where `<file name>` is the filename passed in, without the file extension.
This will generate a few things:

- `missing_lines.csv`: A CSV file containing IDs of any lines that were not present in the line mapping file.

- `res/`: A set of resource patches that should be applied to the original game to produce the fan dub.

Currently, we don't include the SCI code patches necessary to produce the fan dub (to be added in the near future).
If you have the patches already, you will need to rename your `RESOURCE.AUD` file that already exists in the game files to `RESOURCE.SFX`, then copy the remaining files into the game directory.
In the future, we will add a command to be able to build this directly for testing.
Once that has been done, you should be able to start ScummVM with the game in that directory using the following command:

```shell
scummvm -p <path to the game directory> <ScummVM game id>
```

For example, to run Space Quest 5, you would run:

```shell
scummvm -p /path/to/build/spacequest5 sci:sq5
```

Now you can fan dub anything!
