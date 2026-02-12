# Initial Concept
`scitool` is a command line tool for working with Sierra adventure games written in the SCI engine.
Right now, this is mostly targetted for the Space Quest 5 Fan Dub project, so the tools that are
provided are mostly for extracting and analyzing the game's resources.

# Product Definition

## Vision
To provide a robust, developer-centric CLI tool that empowers the SQ5 Fan Dub team and SCI game modders to manipulate game resources efficiently, with a primary focus on automating the fan dubbing workflow and enabling cross-platform resource injection.

## Target Users
- **SQ5 Fan Dub Team Members:** Developers and contributors working on the Space Quest 5 Fan Dub project who need specialized tools for script and audio management.
- **SCI Game Modders:** Enthusiasts who want to modify and re-pack resources for Sierra SCI games.
- **Retro Gaming Preservationists:** Users who analyze and archive game assets for historical and technical documentation.

## Core Goals
- **Fan Dub Workflow Support:** Automate the pipeline from resource extraction to voice actor script generation (CSVs) and final audio compilation.
- **Resource Injection:** Implement the ability to pack modified assets back into original SCI game resource files.
- **Cross-Platform Compatibility:** Ensure the tool maintains a consistent and reliable experience across Windows, macOS, and Linux.

## Key Features
- **Script/Audio Automation:** Specialized commands for extracting game scripts and compiling recorded audio back into the game.
- **Developer-Centric CLI:** Optimized for use in shell scripts and CI/CD pipelines, following standard CLI conventions for easy integration.
- **Modular Resource Handlers:** A modular architecture where each SCI resource type (View, Script, Message, etc.) is handled by a dedicated parser and converter.
