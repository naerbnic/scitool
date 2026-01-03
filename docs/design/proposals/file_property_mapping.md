# Proposal: Configurable Resource Path Patterns

## Problem Statement

`sciproj` needs a flexible way to map file paths to SCI resource identifiers. While the highest level has simple pairs of type and resource ID number, when working with files, a resource may be assembled from several different files. In addition, different projects, or directories within a project, may have different organization conventions for their resources. We need a reliable way of allowing sciproj to find the files that will be used to build a resource for the final build.

## Proposal

We define a way of defining **rules** that assign properties to a file at a given path. The rules are applied to all files in the project, which assigns properties to each file. After assignment is complete, the property assignments are used to figure out which files are used to build each final resource.

### Core Concepts

*   **Rule**: A configuration that assigns a subset of properties to each file path in a project (or none if the file will be ignored).
*   **Property**: A named value assigned to a file path by a rule.
*   **Pattern**: A small glob-like language used to define a set of file paths that will be assigned properties by a rule.

#### Pattern Language

As a primitive operation, a rule applies a set of properties to a subset of files in the project directory tree. To do this, we define a pattern language that specifies a set of file paths that will be assigned properties by a rule, and allows some subset of the path to be extracted into named placeholders.

##### Syntax

A pattern is a string that consists of the following:

- Path separators (`/`). For uniformity on different platforms, we settle on forward slashes, which will map to the platform-specific path separator at build time. As such, the characters `\` and `:` are not used as path separators in patterns. Path separators are not allowed at the start or end of a pattern.

- A multi-character glob (`*`): Matches any number of characters other than the path separator. It should be preceded and followed by either a path separator, a literal character, or the start or end of the pattern. This must match at least one character (so "*.txt" will not match the unix dot file ".txt"). If there is an ambiguity in a glob, such that there are two ways to match a path, this glob does not differentiate between them (e.g. `**/*/**` will match "foo/bar" two different ways. Thus "foo/bar" will match the expression without error.)

- A multi-path glob (`**`): Matches any number of path segments (e.g., "**/src/**"). It must be preceded and followed by either a path separator, or the beginning or end of the pattern.

- A named placeholder (`{field}`): This acts similarly to a glob, but indicates that the matched text can be referenced using the placeholder. Some special named placeholders are reserved as short mnemonics for common properties. Ambiguities created by fields create errors in mappings (e.g. `**/{id}/**` will match "foo/bar" two different ways, but there is an ambiguity if `id` maps to "foo" or "bar", thus it will indicate an error.). A given placeholder can only appear once in a pattern.

- Escaped literals (`\*`, `\**`, `\{`, etc.): A backslash can be used to escape the following character, if it appears literally in a file path that is to be matched. That being said, most of these characters are rarely used in file paths, so this is generally discouraged.

- Literals: All other characters are matched literally.

> NOTE: Backslashes are generally discouraged for several reasons. For example, in config files, they often have to be escaped themselves, which makes them harder to read.

> Q: Do we want to have a specific syntax for matching character sequences that do not include `.`? This would ensure that we could always match a file extension. Without it, we could create ambiguities in dot-separated file segments (at least for placeholders) (e.g. "my.file.pic.png" for the pattern "*.{type}.{ext}" could either match as `type == "file.pic"` and `ext == "png"`, or `type == "file"` and `ext == "pic.png"`, where we desire `type == "pic"` and `ext == "png"`)

##### Examples

###### 1. The "Default" Convention (Type/ID)

`"src/**/{id}.{type}.{ext}"` matches any file in the src directory tree with an ID and type encoded in the filename. For 

###### 2. The "Directory Grouping" Style (Project Specific)
Maps files inside a `.id` suffixed folder to that ID.

`"src/**/room-{id}/{type}/*"` matches src/rooms/room-150/pic/background.aseprite, with `id` bound to "150" and `type` bound to "pic".

#### Properties

A property is a key/value (both being strings) that is assigned to a file path by one or more rules. These properties are then aggregated across the project to define how the final resource is built. For the basic rule definitions, properties are not interpreted, other than detecting ambiguities.

Note that _rules are disjoint additive_. That is, if a file path matches multiple rules, the properties from all matching rules are combined, as long as the properties defined are disjoint. If they are not disjoint, this may cause an error if there is no clear way to resolve the conflict. See the text below for more details.

#### Rules

Rules define a way of mapping file paths to property assignments. It follows the following format (as JSON for explicitness, but TOML will be the final file format):

```json
{
  "name": "rule_name",  // Optional, only necessary if the rule is to be
                        // referenced by another rule.
  "include": [
    // A list of patterns to match file paths with. Note that all patterns must
    // be disjoint, and must all use the same set of placeholders.
    "src/**/{id}.{type}.{ext}",
    "src/rooms/room-{id}/**/*.{ext}"
  ],

  "exclude": [
    // A list of patterns to exclude from the rule. This is useful for
    // excluding files that are not part of the resource, but are in the
    // same directory as the resource.
    //
    // These _must not_ have any placeholders, as they are matched
    // against the file path before any placeholders are expanded.
    "src/docs/**"
  ],

  "properties": {
    "id": "{id}",
    "type": "{type}",
    "file-ext": "{ext}",
    "other": "literal_property",
  },

  "overrides": [
    // References other rules to override. If both this and the given rule
    // match a file path, the properties from this rule take precedence.
    // If this is not specified, if there is a conflict, it will cause an
    // error.
    "base_rule_name"
  ]
}
```

### Rule Definitions

Rules are defined in two places in the project:

1.  In the `sciproj.toml` file in the root of the project.
2.  In `.dir.toml` files in subdirectories of the project.

Rules defined in `.dir.toml` files are scoped to the directory they are defined in, and any subdirectories. Rules defined in `sciproj.toml` are scoped to the root of the project. Patterns in each start from the directory that the file containing the rule is in.

It is possible for rules from different files to match the same file path. 

### Conflict Resolution

Ideally, patterns should be disjoint. However, conflicts may occur where a file path matches multiple rules that attempt to set the same field (e.g. `id`).

We resolve conflicts using a **Strict Priority Hierarchy**:

1.  **Explicit Overrides**: Within a single file (`sciproj.toml` or `.dir.toml`), rules can explicitly name another rule in the same file to override via an `overrides = ["rule_id"]` field. This takes highest precedence.
2.  **Locality (Nesting)**: Rules defined in a `.dir.toml` file deeper in the directory tree (closer to the file) override rules defined in parent directories.
3.  **Error**: If two rules in the *same* configuration file match the same path and conflict on bindings, and neither overrides the other, this is an error. The user must resolve the ambiguity using an explicit override or by refining the patterns.

### Unused Fields

If a resource is defined that declares a pattern with named placeholders, but the resource type does not use those placeholders, we should consider that an error.



### Reversible Mapping (Exports)

While the rules above define how to map files to resources, we often need to do the reverse: given a resource type and ID (and intended properties), where should a new file be created?

Since wildcards cannot be reversed, each rule may optionally define an `export` block.

```json
{
  "name": "standard_resources",
  "include": ["src/**/{id}.{type}.{ext}"],
  "properties": { "id": "{id}", "type": "{type}" },

  // Defines the template for creating new files for this rule.
  // This template is only used if this rule is selected as the 'primary' rule
  // for a given resource type context.
  "export": {
    "path": "src/resources/{type}/{id}.{type}.{ext_default}",
    "defaults": { "ext_default": "scr" }
  },

  // Optional: Mark this as the default rule to use when creating new resources
  // of this type, if no specific rule is requested.
  "primary": true
}
```

### Shared Files Support

Some files need to be mapped with different sets of properties, effectively duplicating their entries in the file property mapping. We manually manage logic to allow for a file entry to be generated based on a separate rule.

This process happens in two passes:
1.  **Discovery Pass**: All files are scanned and properties assigned using the `[[rules]]` defined above.
2.  **Association Pass**: We query the results of the Discovery Pass to find groups of files, and then instantiate "virtual" file entries from defined Templates.

#### 1. Define Templates

Templates are abstract file definitions that can be instantiated multiple times.

```json
{
  "day_palette": {
    "path": "src/palettes/day_palette.ase",
    "properties": { "role": "palette" }
  },
  "night_palette": {
    "path": "src/palettes/night_palette.ase",
    "properties": { "role": "palette" }
  }
}
```

#### 2. Define Associations

Associations query the discovered files and instantiate templates based on the matches.

```json
[
  {
    "description": "Link Day Views to Day Palette",

    // 1. The Trigger (Query)
    // Select files from the Discovery Pass that match these criteria
    "from": ["src/rooms/day/**"],

    // Filter the files by these properties. We use a prefix operator `=` to
    // indicate an exact match.
    "filter": { "role": "=view" },

    // 2. Keying (Group By)
    // Group the matched files by these properties. The 'inject' step will run
    // once for each unique combination of these property values.
    "group_by": ["id", "type"],

    // 3. The Action (Inject)
    // Instantiate the following templates for each group.
    // Properties here can reference the grouped properties using {placeholders}.
    "inject": [
      {
        "template": "day_palette",
        "properties": { "id": "{id}", "type": "{type}" }
      }
    ]
  }
]
```