//! Types for the common symbols that we'll find in a binary script.

/// The symbol of a local function, by local address.
pub struct LocalFunctionSymbol {
    function_name: String,
}

/// A symbol of a u16 class species.
pub struct ClassIdSymbol {
    class_name: String,
}

/// A symbol of a u16 script ID.
pub struct ScriptIdSymbol {}

/// A symbol of an export
pub struct ScriptExportIdSymbol {
    export_name: String,
}

/// A local string defined in the script.
pub struct LocalStringSymbol {
    string: String,
}

pub enum ExternalSymbol {
    ClassId(ClassIdSymbol),
    ScriptId(ScriptIdSymbol),
    ScriptExportId(ScriptExportIdSymbol),
}

pub enum InternalScriptSymbol {
    LocalFunction(LocalFunctionSymbol),
    LocalString(LocalStringSymbol),
}
