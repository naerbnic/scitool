use crate::inputs::text::InputRange;

pub struct Node<T> {
    value: T,
    location: InputRange,
}

pub struct TopLevel {
    items: Vec<Node<Item>>,
}

pub enum Item {
    ScriptNum(Node<ScriptNum>),
    Public(Node<Public>),
    Local(Node<Local>),
    Define(Node<Define>),
    Enum(Node<Enum>),
    Use(Node<Use>),
    Include(Node<Include>),
    Class(Node<Class>),
    Instance(Node<Class>),
    Procedure(Node<Procedure>),
}

pub struct Class {
    // TODO: Add fields
}

pub struct Procedure {
    // TODO: Add fields
}

/// A script number declaration, e.g. `(script# 123)`
pub struct ScriptNum {
    // TODO: Add fields
}

/// A public declaration, e.g. `(public foo bar)`
pub struct Public {
    // TODO: Add fields
}

/// A locals declaration, e.g. `(local foo bar)`
pub struct Local {
    // TODO: Add fields
}

/// A definition item, e.g. `(define FOO 3)`
pub struct Define {
    // TODO: Add fields
}

/// An enum definition, e.g. `(enum 8 FOO BAR)`
pub struct Enum {
    // TODO: Add fields
}

/// A use definition, e.g. `(use "MyScript")`
pub struct Use {
    // TODO: Add fields
}

/// An external file include, e.g. `(include "game.sh")`
pub struct Include {
    // TODO: Add fields
}
