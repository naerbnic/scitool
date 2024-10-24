# The SCI11 Script/Heap Resource Format

This doesn't seem to be documented anywhere else, aside from reading the
ScummVM source code, and is different from the format described in
https://wiki.scummvm.org/index.php?title=SCI/Specifications/SCI_virtual_machine/Introduction

These notes are the best of my understanding at the moment, and will change
as I understand this better.

> Note: There are some structural requirements that are needed by the ScummVM implementation, even
though it appears that there is no absolute need to be structured in this style. For instance,
the Heap doesn't require us to detect the location of the string buffer, as I believe these will
be handled by pointers from the script section after relocation.
>
> As such, we we need to follow the exact order specified by ScummVM. This is still
> not too difficult, as this is probably a result of the original implementation of
> the compiler, but it may be useful to move this into ScummVM to simplify loading/processing
> of scripts.

## Relocation Block Format

Both of the Heap and Script resources use a relocation record format. The first
`u16` in the block is a reference to the start of a relocation block, which
gives offsets in the block that need to have the `heap_offset` be added to
it before returning. These offsets may not be word aligned.

## Heap Resource Format

```rust
struct HeapResource {
    reloc_block_ptr: *RelocBlock,
    num_locals: u16,
    local_values: [u16; num_locals],
    object_stream: [Object; <until zero magic code>],
    string_buffer: [u8; <until reloc_block_ptr>],
}

struct RelocBlock {
    num_relocs: u16,
    reloc_entry: [*u16; num_relocs]
}

struct Object {
    /// 0x1234
    magic_number: u16,
    /// Words from start of object
    object_size: u16,
    /// A pointer to a list of selectors for this object's properties.
    /// 
    /// This appears to often be the same as method_record_offset in objects.
    /// This may also be a pointer marker, where the var selectors are between
    /// var_selector_offset and method_record_offset. This should be preserved.
    var_selector_offset: *[Selector; object_size - 5],
    /// A pointer to a MethodRecord location, in the script portion.
    method_record_offset: *MethodRecord,
    /// Apparently a padding and/or reserved_value, should be 0.
    _padding: u16
    /// Default property values
    /// 
    /// There are some (fairly) universal properties in here, used for
    /// various loading properties:
    /// 
    /// properties[0]: The class species (class table ID), or -1 if an object
    /// properties[1]: The parent class of this object/class (probably class species)
    /// properties[2]: Flags for the object:
    ///   0x8000: If set, object is a class
    ///   0x0001: ?? (called Clone flag in SCICompanion).
    /// properties[4]: The name of the class/object
    ///   Apparently this is sometimes not actually the name, and the class
    ///   structure has to be checked.
    properties: [u16; object_size - 5]
}
```

### Script Resource Format

```rust
struct ScriptResource {
    reloc_block_ptr: *RelocBlock,
    num_exports: u16,
    exports_offsets: [u16; num_exports],
}
```