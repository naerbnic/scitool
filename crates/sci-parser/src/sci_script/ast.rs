use crate::inputs::text::InputRange;

pub struct Node<T> {
    #[expect(dead_code)]
    value: T,
    #[expect(dead_code)]
    location: InputRange,
}

pub struct TopLevel {
    #[expect(dead_code)]
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

pub enum ClassKind {
    Instance,
    Class,
}

pub struct Class {
    #[expect(dead_code)]
    kind: Node<ClassKind>,
    #[expect(dead_code)]
    name: Node<String>,
    #[expect(dead_code)]
    base_class: Option<Node<String>>,
    #[expect(dead_code)]
    properties: Vec<Node<Property>>,
    #[expect(dead_code)]
    methods: Vec<Node<Method>>,
}

pub struct Property {
    #[expect(dead_code)]
    name: Node<String>,
    #[expect(dead_code)]
    value: Node<Expr>,
}

pub struct Method {
    #[expect(dead_code)]
    name: Node<String>,
    #[expect(dead_code)]
    params: Vec<Node<String>>,
    #[expect(dead_code)]
    temp_vars: Vec<Node<TempDecl>>,
    #[expect(dead_code)]
    body: Vec<Node<Statement>>,
}

pub enum TempDecl {
    /// A plain temporary variable, e.g. `foo`
    TempVar(Node<String>),

    /// A temporary array declaration, e.g. `[foo 10]`
    TempArray(Node<String>, Node<u16>),
}

pub enum AssignOp {
    Direct,
    BinOp(BinOp),
}

pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    Eq,
    NEq,
    Lt,
    ULt,
    Le,
    ULe,
    Gt,
    UGt,
    Ge,
    UGe,
}

pub enum UnaryOp {
    Negate,
    BoolNot,
    BitNot,
}

pub enum InPlaceOp {
    Increment,
    Decrement,
}

pub enum Statement {
    Return(Node<Expr>),
    /// A plain expression, whose value is discarded.
    Expr(Node<Expr>),
}

pub enum Expr {
    Assign(Node<AssignOp>, Node<String>, Node<Box<Expr>>),
    BinOp(Node<BinOp>, Node<Box<Expr>>, Node<Box<Expr>>),
    InPlaceOp(Node<InPlaceOp>, Node<String>),
    UnaryOp(Node<UnaryOp>, Node<Box<Expr>>),
}

pub struct Procedure {
    #[expect(dead_code)]
    name: Node<String>,
    #[expect(dead_code)]
    params: Vec<Node<String>>,
    #[expect(dead_code)]
    temp_vars: Vec<Node<TempDecl>>,
    #[expect(dead_code)]
    body: Vec<Node<Statement>>,
}

/// A script number declaration, e.g. `(script# 123)`
pub struct ScriptNum {
    #[expect(dead_code)]
    num_expr: Node<Expr>,
}

/// A public declaration, e.g. `(public foo bar)`
pub struct Public {
    #[expect(dead_code)]
    names: Vec<Node<String>>,
}

/// A locals declaration, e.g. `(local foo bar)`
pub struct Local {
    #[expect(dead_code)]
    names: Vec<Node<String>>,
}

/// A definition item, e.g. `(define FOO 3)`
pub struct Define {
    #[expect(dead_code)]
    name: Node<String>,
    #[expect(dead_code)]
    value: Node<Expr>,
}

/// An enum definition, e.g. `(enum 8 FOO BAR)`
pub struct Enum {
    #[expect(dead_code)]
    name: Node<String>,
    #[expect(dead_code)]
    start: Option<Node<Expr>>,
    #[expect(dead_code)]
    items: Vec<Node<String>>,
}

/// A use definition, e.g. `(use "MyScript")`
pub struct Use {
    // TODO: Add fields
}

/// An external file include, e.g. `(include "game.sh")`
pub struct Include {
    // TODO: Add fields
}
