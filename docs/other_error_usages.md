# Usages of `OtherError` as an Internal Error Type

This document lists places in the codebase where `OtherError` is used to represent internal errors, validation failures, or ad-hoc error conditions, typically via `ensure_other!`, `bail_other!`, `OtherError::from_msg`, or `ok_or_else_other`.

## `crates/scidev/src/resources/file/patch.rs`

* **Line 98**: `ensure_other!(content_res_type == res_type, ...)`
  * **Context**: Validates that the resource type in the patch file matches the expected resource type.
* **Line 153**: `ensure_other!(data.len() <= 127, ...)`
  * **Context**: Validates that simple extra data in a patch file does not exceed 127 bytes.
* **Line 168**: `ensure_other!(ext_header.len() == 24, ...)`
  * **Context**: Validates that the extended header size in a patch file is exactly 24 bytes.

## `crates/scidev/src/resources/types/audio36.rs`

* **Line 170**: `ensure_other!(format == sample.format, ...)`
  * **Context**: Validates that the audio format of a new sample matches the existing format when adding an entry.

## `crates/scidev/src/resources/types/msg.rs`

* **Line 157**: `ensure_other!(offset as usize <= msg_res.size(), ...)`
  * **Context**: Validates that a string offset is within the bounds of the message resource.
* **Line 201**: `bail_other!("Unsupported message resource version: {}", version_num)`
  * **Context**: Returns an error if the message resource version is not supported (only version 4 is supported).

## `crates/scidev/src/script_loader.rs`

* **Line 98**: `.ok_or_else_other(|| "Selector table not found")`
  * **Context**: Returns an error if the selector table resource (Vocab 997) is missing.
* **Line 110**: `.ok_or_else_other(|| "Selector heap not found")`
  * **Context**: Returns an error if the heap resource for a script is missing.

## `crates/scidev/src/script_loader/mem_loader.rs`

* **Line 66**: `OtherError::from_msg("Relocation block size and length must match")`
  * **Context**: Validates that the relocation block size matches the expected length.
* **Line 126**: `OtherError::from_msg("Invalid object magic number")`
  * **Context**: Validates the magic number of an object in the heap.
* **Line 182**: `OtherError::from_msg("Relocation offset out of bounds")`
  * **Context**: Validates that a relocation offset is within the bounds of the data.

## `crates/scidev/src/utils/mem_reader.rs`

* **Line 137**: `OtherError::from_msg(message.into().into_owned())`
  * **Context**: Helper method `create_invalid_data_error_msg` to create an invalid data error with a string message.
* **Line 409**: `OtherError::from_msg(message.into())`
  * **Context**: Helper method `err_with_message` to create a `MemReaderError` with a string message.

## `crates/scidev/src/utils/testing/block.rs`

* **Line 23**: `OtherError::from_msg(...)`
  * **Context**: Validates that the entire buffer was consumed during parsing in a test helper.
