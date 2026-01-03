# Compiler File Pipeline Design

The compiler is a tool that takes in some input files, and produces some output
each time the tool is run with the same arguments. This doc is intended to
describe the different categories of input and output files, and define
the logical contents of the different files. Note that this does not actually
define the file formats, but rather what data they contain.

## Input Categories

### Original Game Resources

When compiling as a patch on top of an existing game, the resources of the game
are used to ensure compatibility for the resulting patches. Some resources are
not needed by the compiler, as they would not change the behavior (e.g.
graphics resources), but some resources are critical.

These resources are not expected to change during the progress of the project.
For any compiler inputs/outputs that depend on these resources not changing, we
should record a hash of the resource contents (e.g. SHA-256) to ensure that we
can detect any changes at compile time.

The following resources are needed by the compiler.

#### Selector Table

`Vocab.997` is a resource that contains a mapping from an integer (`u16`) to
an ASCII name, which is used in many places as runtime identifiers. This
includes object property and method names. If any script resources are copied
from the original game, the valid entries in this mapping MUST be preserved
in any output selector table. These symbols must also be used for the same names
in new and/or changed scripts.

#### Species Table

`Vocab.996` is a resource that contains a mapping from a class species (i.e. a
class ID) to which script contains its definition.

#### Original Game Script/Heap Resources

In SCI 1.1, scripts are split into two pieces: The Script resource, and the
Heap resource, which share the same resource number. There are interactions
between the resources, and how they're mapped, but for the purposes of this doc,
we consider the pair of resources as a single entity.

Scripts contain the following data that is readable by the compiler:

- `Export[n]`: A list of exported symbols. These are address references to
  either a function contained in the script, or an object defined in the script.
  For instance, Room scripts in SQ5 by convention export the main room object
  as `Export[0]`.
- `Class[species]`: A mapping of species numbers to the classes defined in the
  script. Note that the Species Table above could be rebuilt from these entries
  in the script resources, but is compiled separately in order to make lookups
  speedy. This information includes:
  
  - Species Number
  - Class Name
  - Properties (selectors and default values)
  - Methods (selectors)

The script resource `Script.0` is special, as it is considered the entry point
for the program, and in effect the "global" script. The locals of this script
are accessible from all other scripts as Global indexes. Thus we have the
following for that script specifically:

- `Local[n]`: The set of local variables used within the script. Aside from
  the Global script `Script.0`, these are not accessible to other scripts.
  These contain 16-bit integral values (whether they're signed or unsigned
  depends on usage). These can also contain pointers to other objects.
  (NOTE: I'm a little confused, as it looks like these locals can also be
  referenced by address, in which case they are treated as 2 byte values, which
  is not sufficient to contain a full far pointer).

Details about the output script/heap resources will be discussed in the
output section.

### Source Files

These consist of the product of the project. They are expected to be committed
to version control, and as such MUST work as human-readable line-oriented
text files, so that appropriate diffing and merging can be performed.

Note that this set is somewhat different than the existing types of source
files used by SCI Companion and SCI Studio. We will address the differences
when needed.

#### SCI Script files (`*.sc`)

These are the typical source files as used by SCI Companion. We will by
convention use the script format used by default by SCI Companion, instead of
the SCI Studio syntax.

These scripts define all of the implementation of a resulting Script/Heap file pair,
with some need to be able to reference and link to external resources. A script
must be able to reference:

- Class Names: These resolve to a species defined either within the same script,
  or a separate script.
- Selector Names: These resolve to a selector ID that is used to resolve object
  message sends (e.g. sending methods, accessing or setting properties, etc.)
- Global Variable Names: Access to global variables. These map to local
  variables in `Script.0`.
- Script names/numbers: When trying to resolve exported objects or functions
  in an external script, SCI script uses module names instead of resource
  numbers.

Some of these entities are not provided direction in the original resources,
and depend on other source files to be resolved.

#### SCI Script Header Files (`*.sch`)

These are header files that can be included directly in a script.

#### SCI Script Resource Interface Files (`*.scri`)

These are a new source file format for resources that are intended to not be
modified in the patch generated by this source code. These provide names and
external resources for each script that is referenced in the final patch.

The name of these files are used as module names, but the script number they
map to are included within the data of the file.

Each file contains the following:

- The script number from the original resources this file maps to
    - It is an error to have two `*.scri` files map to the same script number
- An SHA-256 hash of the original resources. This can verify that the interface
  still applicable to the same original resources.
- Names for each export from the script
- A mapping from local variable indexes to names
    - This is only used to import symbols from the global main file, and may not
      be included otherwise.

In order to ensure that we can always find globals, we require the global
script resource interface file to be called `main.scri`, if `main.sc` does
not exist. It is a compile error if this does not have resource number 0.

It is not necessary for every resource in the original game to have a `*.scri`
file, but if a `*.sc` file references a module with a name that is not another
source file, it must exist.

`*.scri` files can be at least partially generated from the original game script
resource, but only with generic symbols for the exports and locals. It would
be intended to have these files generated once when the project is created, and
then edited as a source file.

In some ways, these files are similar to the previous `*.sco` files generated
by SCI Companion, but with some notable differences:

- It does not contain information that can be obtained directly from the
  original script resource aside from the hash. The compiler is expected to
  obtain that data directly from the original resources.
- The file format will be in a human-readable text-based file format, which
  is friendly to be committed into version control.

## Output Categories

There are two general categories of output for a compiler (aside from
intermediate files that are used for internal compiler purposes). These output
files should always be reliabily generated if the state of the inputs is the
same

### Patch Outputs

These are files that will be directly or indirectly distributed as part of the
output script patch. They may be used as input to a resource builder, which
recreates the `resource.map` and `resource.000` files with the additional
patches, or adds the patches as external file patches via the existing SCI
engine file patch feature.

#### Manifest file (`manifest.json`)

This is a file that describes the full contents of the patch, and other files
in the output that are part of the patch. This also includes data on which
scripts we depend on from the original game files, and can be used with that
data to rebuild a new resource source for the game.

The data that is contained in the manifest:

- An entry for each numbered Script/Heap pair
    - `id`: The script ID (as an integer) for the script.
    - If the entry is taken from the original data
        - `hash`: The SHA-256 hash of the script/hash resources
    - If the entry is built by the compiler
        - `script`/`heap`: Filenames of the respective script and heap raw
          resource files.

#### Script/Heap files

These are files that contain the raw resource data for the patch. Their names
will be contained in the manifest.

### Debug Outputs

These are outputs that are generated alongside compiled outputs to keep track
of debug information, as can be used by other tools. They can be discarded
without breaking the patch, but tools may use it to be able to map things like
line numbers to offsets within the output files.