# SCITool CLI notes

## Resource Discovery & Inspection

```shell
scitool res list [--type TYPE] [--detailed]
scitool res info <type> <id>  # Show metadata, dependencies, format info
scitool res validate <type> <id>  # Check resource integrity
```

## Resource Extraction & Conversion

```shell
scitool res extract <type> <id> [--format FORMAT]  # Extract to various formats
scitool res convert <input> <output> [--target-format FORMAT]
```

## Resource Patching

```shell
scitool res patch create <type> <id> <source-file>
scitool res patch apply <patch-file>
scitool res patch list  # Show applied patches
```

## Script Resources

```shell
scitool script decompile <id> [--output DIR]
scitool script compile <source> [--output FILE]
scitool script analyze <id>  # Show exports, classes, dependencies
```

## Message/Text Resources

```shell
scitool msg extract <id> [--format csv|json|txt]
scitool msg search <pattern> [--room ID] [--verb ID]
scitool msg stats  # Show usage statistics
```

## Audio Resources

```shell
scitool audio extract <id> [--format wav|ogg]
scitool audio info <id>  # Show sample rate, format, etc.
scitool audio convert <input> <output>
```

## Graphics Resources

```shell
scitool gfx extract <type> <id> [--format png|bmp]
scitool gfx info <type> <id>  # Show dimensions, palette info, cel count
scitool gfx convert <input> <output>
```
