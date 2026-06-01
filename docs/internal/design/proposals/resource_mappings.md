# Mapping Workspace Files to SCI Resources

In [our file property mapping proposal](file_property_mapping.md), we define a way of mapping file paths to properties. This proposal extends that to define a general scheme of how input files and their properties are used as input to build SCI resources.

## Core Concepts

- **Resource**: A low-level SCI resource, such as a View, Script, or MessageMap.
- **Resource Type**: The type of a resource, such as "View", "Script", or "MessageMap".
- **Resource ID**: The ID of a resource, such as "150" or "room-150".
- **Sub-Resource**: A logical piece of data that is part of a resource, such as a View's palette, or a single loop (or cel) of a view.
- **Resource Builder**: An abstract operation that converts a collection of files (with their attached properties) into a single SCI resource.
- **File Role**: The way a specific file is intended to be used as an input to a resource builder. This is independent of the file's path, and even its extension. For example, an aseprite file could be used to encode all of the loops of a view, one loop, or even a single cel (each of which would be a different file role).
- **File Entry**: A file path and a set of properties (string key/value pairs).
- **File Collection**: A set of file entries. Note that the same file path can appear in the collection multiple times, with different properties.

## Resource Mapping Properties

As output from the file property mapping system, we have a set of file paths, each with a set of properties. Two of these properties are used specially by the resource mapping system:

- `type`: The type of the resource, such as "View", "Script", or "MessageMap". These are the same as the resource types, and can be canonicalized to the SCi resource type names (e.g. "V56" and "View" map to the same logical type.)
- `id`: The numeric ID of the resource, such as "150" or "room-150". This value is normalized from a string to a number (dropping initial 0s) and using an equivalent of atoi() to convert to a number. If after removing initial 0s the string is not a valid decimal number, an error is raised.

After the file property mapping system has run and generated a set of file entries, we aggregate the entries by canonical type and numeric ID, creating a single file collection for each resource type/ID pair. At this point, each file collection is given to the resource builder to figure out how to build the resource from the files.

Multiple resource builders are allowed for a given resource type. Which to use needs to be able to be determined from the file collection passed to it. The decision must be made only looking at the file paths and properties, and not at the file contents.

An additional special property `role` is used to determine how each file is to be used by the resource builder, which can be used to indicate which builder is to be used. More than one file in a collection can map to the same role, but either those files must be order independent, or the files must have additional properties to disambiguate them.

When a file collection has only a single file, and the resource builder has a direct mapping from a specific file type to the resource, the `role` property can be omitted.

Note that, additionally, a single resource builder can have multiple ways that different subresources are encoded. For example, while it's possible for a message map to be specified using a single text file per line, it's also possible to specify it as a CSV file, or other file format. In this case, the `role` property is used to determine which format to use.

Subresource files in the file collection with a given role can be identified with other properties that are specific to the intended resource builder. For instance, voice audio files can be mapped to specific noun, verb, condition, and sequence numbers within a room using their respective properties.

If multiple resource builders attempt to claim a resource to build from a file collection, an error is raised.

