#![expect(dead_code, reason = "Development in progress")]

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct EnumType {
    values: Vec<String>,
}

#[non_exhaustive]
#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) enum AtomType {
    Integer,
    String,
    DateTime,
    Bool,
    Enum(EnumType),
}

impl AtomType {
    fn into_set(self) -> ColumnType {
        ColumnType::Set(self)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) enum ColumnType {
    // A single value column.
    Single(AtomType),
    Set(AtomType),
}

impl From<AtomType> for ColumnType {
    fn from(value: AtomType) -> Self {
        ColumnType::Single(value)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct TableSchema {
    columns: Vec<ColumnSchema>,
    /// A unique constraint that each entity must satisfy, and can uniquely
    /// determine the entity.
    ///
    /// All columns used in the constraint must not be nullable.
    primary_key: UniqueConstraint,

    /// Other constraints on the data.
    constraints: Vec<Constraint>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct ColumnSchema {
    /// The name of the column. Must be a valid C-style ASCII identifier.
    ///
    /// Used to reference this column elsewhere.
    name: String,

    /// A description of the column, if available.
    #[serde(default)]
    description: Option<String>,

    /// The kind of values that are in this column.
    value_type: ColumnType,

    /// Whether entities can be missing this column.
    #[serde(default)]
    nullable: bool,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) enum Constraint {
    /// A constraint indicating a set of fields that must be unique
    /// for every entity.
    Unique(UniqueConstraint),
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct UniqueConstraint {
    /// The fields in the table that must collectively be
    /// unique across the whole table.
    fields: Vec<String>,
}
