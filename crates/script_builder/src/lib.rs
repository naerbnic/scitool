pub mod symbols;

pub struct StringRef(u32);
pub struct ExportRef(u32);
pub struct ObjectRef(u32);
pub struct FunctionRef(u32);
pub struct SelectorRef(u32);
pub struct ClassRef(u32);

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

pub trait ObjectBuilder {
    type MethodBuilder: FunctionBuilder;
    fn set_property(&mut self, name: &str, value: &Value);
    fn add_method_impl(&mut self, selector: &str, body: impl FnOnce(&mut Self::MethodBuilder));
}

pub trait FunctionBuilder {}

pub trait ScriptBuilder {
    type FunctionBuilder: FunctionBuilder;
    type ObjectBuilder: ObjectBuilder;
    fn add_string(&mut self, string: &str) -> StringRef;
    fn add_export(&mut self, index: Option<u16>, value: &Value) -> ExportRef;
    fn build_function(
        &mut self,
        name: &str,
        body: impl FnOnce(&mut Self::FunctionBuilder),
    ) -> FunctionRef;
    fn build_object(&mut self, body: impl FnOnce(&mut Self::ObjectBuilder)) -> ObjectRef;
    fn build_class(&mut self, name: &str, body: impl FnOnce(&mut Self::ObjectBuilder)) -> ClassRef;
}
