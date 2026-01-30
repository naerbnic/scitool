---
description: Re-examines and improves comments in a specified file according to the commenting skill guidelines.
---

1. Read the commenting guidelines by viewing `.agent/skills/comments/SKILL.md`.
2. Read the target file specified by the user. If no file is specified, ask the user which file they would like to recomment.
3. Analyze the code and existing comments in the target file against the guidelines in `SKILL.md`:
    * **Documentation Comments (`///`, `//!`)**:
        * Ensure all public items (structs, enums, functions, modules) have documentation comments.
        * Verify that documentation focuses on *purpose* and *usage* (the "contract"), not implementation details.
        * Check that `unsafe` functions/traits have a `# Safety` section describing caller obligations.
    * **Developer Comments (`//`)**:
        * Ensure comments explain *why* something is done (intent, constraints, complex logic), not just *what* the code does.
        * Check that `unsafe` blocks are preceded by `// SAFETY:` comments explaining why the operation is safe.
    * **Cleanup**:
        * Remove comments that merely restate the code (e.g., `// Increment i`).
        * Update incorrect or outdated comments.
4. Apply the improvements to the file using `replace_file_content` (for single contiguous blocks) or `multi_replace_file_content` (for multiple scattered changes).
    * **CRITICAL**: Do NOT change the code logic. Only add, modify, or remove comments.
5. Verify that the file still compiles (if applicable/feasible) or that the changes look correct.
