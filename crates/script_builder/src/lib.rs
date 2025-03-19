use sci_utils::{
    reloc_buffer::{RelocatableBuffer, writer::RelocWriter as _},
    symbol::{Symbol, WeakSymbolMap},
};

pub mod linker;

#[derive(Clone)]
pub struct StringRef(#[expect(dead_code)] Symbol);
#[derive(Clone)]
pub struct ExportRef(#[expect(dead_code)] Symbol);
#[derive(Clone)]
pub struct ObjectRef(#[expect(dead_code)] Symbol);
#[derive(Clone)]
pub struct FunctionRef(#[expect(dead_code)] Symbol);
#[derive(Clone)]
pub struct SelectorRef(#[expect(dead_code)] Symbol);
#[derive(Clone)]
pub struct ClassRef(#[expect(dead_code)] Symbol);

pub enum Value {
    // A 16-bit integer.
    //
    // This is used for both signed and unsigned integers.
    Int16(u16),
    // A local string defined in the script.
    String(StringRef),
    // A reference to an object.
    Object(ObjectRef),
    // A reference to a class.
    Class(ClassRef),
    // A reference to a function.
    Function(FunctionRef),
}

struct Export {
    index: Option<u16>,
    value: Option<Value>,
}

#[expect(dead_code)]
struct MethodDef {
    name: SelectorRef,
    function: FunctionRef,
}

struct FunctionDef {}

#[expect(dead_code)]
struct PropertyRef {
    name: SelectorRef,
    value: Value,
}

#[expect(dead_code)]
struct LocalClassDef {
    parent: Option<ClassRef>,
    name: Option<StringRef>,
    species: Option<u16>,
    properties: Vec<PropertyRef>,
    methods: Vec<MethodDef>,
}

#[expect(dead_code)]
struct LocalObjectDef {
    parent: ClassRef,
    name: Option<StringRef>,
    properties: Vec<Value>,
    methods: Vec<MethodDef>,
}

/// Class declarations that are external to this script.
enum ClassDef {
    /// A numbered class species, from other decompiled scripts.
    #[expect(dead_code)]
    Species(u16),
}

pub struct ScriptBuilder {
    strings: WeakSymbolMap<RelocatableBuffer>,
    exports: WeakSymbolMap<Export>,
    #[expect(dead_code)]
    class_defs: WeakSymbolMap<ClassDef>,
    #[expect(dead_code)]
    local_functions: WeakSymbolMap<FunctionDef>,
}

pub struct ExportBuilder<'a> {
    export_ref: &'a mut Export,
}

impl ExportBuilder<'_> {
    pub fn set_index(&mut self, index: u16) {
        self.export_ref.index = Some(index);
    }

    pub fn clear_index(&mut self) {
        self.export_ref.index = None;
    }

    pub fn set_value(&mut self, value: Value) {
        self.export_ref.value = Some(value);
    }
}

impl ScriptBuilder {
    pub fn new() -> Self {
        Self {
            strings: WeakSymbolMap::new(),
            exports: WeakSymbolMap::new(),
            class_defs: WeakSymbolMap::new(),
            local_functions: WeakSymbolMap::new(),
        }
    }

    pub fn add_string(&mut self, string: &str) -> StringRef {
        assert!(string.is_ascii());
        let string_sym = Symbol::with_name(format!("script string {:?}", string));
        let mut reloc_builder = RelocatableBuffer::builder();
        reloc_builder.mark_symbol(string_sym.clone());
        reloc_builder.write_bytes(string.as_bytes());
        reloc_builder.write_u8(0);
        self.strings.insert(
            &string_sym,
            reloc_builder
                .build()
                .expect("String buffer builds successfully"),
        );
        StringRef(string_sym)
    }

    pub fn add_export(&mut self, value: Value) -> (ExportRef, ExportBuilder) {
        let export_sym = Symbol::new();
        let export = Export {
            index: None,
            value: Some(value),
        };
        let builder = match self.exports.try_insert_mut(&export_sym, export) {
            Ok(export_ref) => ExportBuilder { export_ref },
            Err(_) => panic!("Export symbol already exists"),
        };
        (ExportRef(export_sym), builder)
    }

    pub fn declare_class_species(&mut self, _species: u16) -> ClassRef {
        todo!()
    }
}

impl Default for ScriptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
