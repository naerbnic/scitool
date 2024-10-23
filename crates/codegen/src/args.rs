#[derive(Clone, Copy, Debug)]
pub enum OpcodeWidth {
    Byte,
    Word,
}

#[derive(Clone, Copy, Debug)]
pub struct ScriptNum(u16);
#[derive(Clone, Copy, Debug)]
pub struct DispatcherIndex(u16);

#[derive(Clone, Copy, Debug)]
pub struct AbsoluteAddress(u16);

#[derive(Clone, Copy, Debug)]
pub struct Label<T>(T);