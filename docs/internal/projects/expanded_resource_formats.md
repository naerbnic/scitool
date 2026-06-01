# File Formats for Expanded Resources

As the resource volumes for SCI games are made up of proprietary data formats that are targetted at the engine, in order to develop a game, we have to be able to turn those resources into file formats that are editable by other pieces of media editing software. In a few cases this is quite easy, such as audio formats that can be fairly freely converted from others (even if it has to be downsampled to the game's sample frequencey, etc.). Other times this is difficult, as the data is either somewhat obsolete, such as working with paletted images that need shared palettes. This is a list of the different formats that I am aware of, and a description of how they will map to editable files.

Note that while it is convenient to have file types that can easily be used and merged within a VCS, it is more important here that the files are easily editable. Some file formats will inherently not be easy to merge, such as any image format, and this is acceptable for our use case. In terms of priority for file format selection:

1. Editability with common external tools.
2. Directly usable by scidev/tools; Does not require an export pipeline.
3. Is round-trippable with the original raw resource data (without losing metadata on edits).
4. Is able to be merged with a VCS.

## Code Resources (Script, Heap, Vocab:996, Vocab:997)

These will be managed by a separate compiler. In a game modification, some of these have to be extracted from the original resources, but generally they can be overridden with newly compiled scripts without too much worry.

## Views

Views are a format that are for entity images and animations. They consist of a list of `loop`s, each with a list of `cel`s, each of which are single graphics images. It appears that all cels within a view share the same palette, which can also be embedded in the resource. Each cel also has an offset that determines its relative position, to allow for various sizes of image to line up when animated.

For file formats, it seems that we would need to be able to edit each animation separately, but have to be able to order each of the animations in the compiled view, and ensure that all animations have the same palette. It looks like .aseprite files are the best format to represent this.

## Pics

These are generally used as background images, and can include pixel and/or vector components (Allowing pixel data from SCI1.1). Pixel data can contain transparent pixels for the purposes of overlays, and . Pixel data is similar to cel data above, and vector data is a sequence of draw commands encoded in a small bytecode.

For the static pixel data, it makes sense to use PNG or the like as the output format, as it can hold paletted data, along with the palette itself, but I don't know how to overlay vector data as well. Perhaps some variant on SVG?

Q: What formats are possible here? Advantages and disadvantages?

## Text

This is a concatenated sequence of strings, separated at least by zero bytes. They are indexed starting at 0, and counted sequentially.

As this is just text data, either something like JSON, YAML, or another structured data format should be appropriate for this formatting. Compilation would have to ensure that the strings are usable as ASCII.

## Font

Fonts are a sequence of bitmaps of individual characters, mapping from a contiguous sequence of indexes to the character bitmaps. Each is a 1-bit image, with a separately specified height and width, with an overall height of the character.

One possibility is to use the aseprite file format for this as well. There has to be some validation on building, but if each frame is indexed by the frame, and there is some way to indicate the overall height of the character. Link frames can be used to reuse sprite data for multiple characters.
