#![expect(
    clippy::todo,
    unreachable_pub,
    dead_code,
    reason = "Development in progress"
)]

mod serde_types;

use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet, btree_map},
};

use polars::{prelude::*, series::builder::SeriesBuilder};
use scidev_errors::{AnyDiag, bail, define_error, diag, ensure, in_err_context, prelude::*};

define_error! {
    pub struct TableError;
}

pub(crate) struct SchemaBuilder {}

impl SchemaBuilder {
    pub fn new() -> Self {
        todo!()
    }
}

#[derive(Clone, Debug, ::serde::Deserialize)]
pub(crate) struct EnumType {
    values: Vec<String>,
}

#[non_exhaustive]
#[derive(Clone, Debug, ::serde::Deserialize)]
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

#[derive(Clone, Debug, ::serde::Deserialize)]
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

struct InnerTableSchema {
    columns: BTreeMap<ColumnId, ColumnSchema>,
    /// A unique constraint that each entity must satisfy, and can uniquely
    /// determine the entity.
    ///
    /// All columns used in the constraint must not be nullable.
    primary_key: UniqueConstraint,

    /// Other constraints on the data.
    constraints: Vec<Constraint>,
}

pub(crate) struct TableSchema(Arc<InnerTableSchema>);

impl TableSchema {
    pub(crate) fn builder() -> TableSchemaBuilder {
        TableSchemaBuilder {
            columns: BTreeMap::new(),
            unique_constraints: BTreeSet::new(),
        }
    }

    pub(crate) fn create_table(
        &self,
        items: impl IntoIterator<Item = impl Borrow<Entity>>,
    ) -> Result<Table, TableError> {
        Ok(Table {
            schema: self.clone(),
            frame: create_data_frame(&self.0, items)?,
        })
    }
}

impl Clone for TableSchema {
    fn clone(&self) -> Self {
        TableSchema(Arc::clone(&self.0))
    }
}

pub(crate) struct ColumnSchema {
    /// The name of the column. Must be a valid C-style ASCII identifier.
    ///
    /// Used to reference this column elsewhere.
    name: ColumnId,

    /// A description of the column, if available.
    description: Option<String>,

    /// The kind of values that are in this column.
    value_type: ColumnType,

    /// Whether entities can be missing this column.
    nullable: bool,

    // The polars type for the column.
    polars_type: DataType,

    // The creator of the polars values for this column.
    value_creator: ValueCreator<Value>,
}

#[derive(Clone)]
pub(crate) enum Constraint {
    /// A constraint indicating a set of fields that must be unique
    /// for every entity.
    Unique(UniqueConstraint),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct UniqueConstraint {
    /// The fields in the table that must collectively be
    /// unique across the whole table.
    fields: BTreeSet<ColumnId>,
}

fn normalize_float(f: f64) -> f64 {
    if f.is_nan() {
        f64::NAN
    } else if f == 0.0 {
        0.0
    } else {
        f
    }
}

#[derive(Clone, Debug)]
pub(crate) enum AtomValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

impl AtomValue {
    fn discriminant_idx(&self) -> u8 {
        match self {
            Self::Bool(_) => 0,
            Self::Integer(_) => 1,
            Self::Float(_) => 2,
            Self::String(_) => 3,
        }
    }

    fn from_any_value(v: AnyValue) -> Self {
        match v {
            AnyValue::Boolean(b) => Self::Bool(b),
            AnyValue::UInt8(n) => Self::Integer(n.into()),
            AnyValue::UInt16(n) => Self::Integer(n.into()),
            AnyValue::UInt32(n) => Self::Integer(n.into()),
            AnyValue::Int8(n) => Self::Integer(n.into()),
            AnyValue::Int16(n) => Self::Integer(n.into()),
            AnyValue::Int32(n) => Self::Integer(n.into()),
            AnyValue::Int64(n) => Self::Integer(n),
            AnyValue::Float32(f) => Self::Float(f.into()),
            AnyValue::Float64(f) => Self::Float(f),
            AnyValue::String(s) => Self::String(s.to_string()),
            AnyValue::StringOwned(s) => Self::String(s.into()),
            AnyValue::Enum(i, e) => Self::String(e.cat_to_str(i).unwrap().to_string()),
            AnyValue::EnumOwned(i, e) => Self::String(e.cat_to_str(i).unwrap().to_string()),
            AnyValue::Datetime(t, tu, tz) => {
                assert!(tz.is_none_or(|tz| tz == &polars::datatypes::TimeZone::UTC));
                let date_time = match tu {
                    TimeUnit::Nanoseconds => chrono::DateTime::from_timestamp_nanos(t),
                    TimeUnit::Microseconds => chrono::DateTime::from_timestamp_micros(t).unwrap(),
                    TimeUnit::Milliseconds => chrono::DateTime::from_timestamp_millis(t).unwrap(),
                };
                Self::String(date_time.to_rfc3339())
            }
            _ => panic!("Unsupported value type for AtomValue"),
        }
    }
}

impl PartialEq for AtomValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => {
                normalize_float(*a).to_bits() == normalize_float(*b).to_bits()
            }
            (Self::String(a), Self::String(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for AtomValue {}

impl PartialOrd for AtomValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AtomValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let self_disc = self.discriminant_idx();
        let other_disc = other.discriminant_idx();
        match self_disc.cmp(&other_disc) {
            std::cmp::Ordering::Equal => match (self, other) {
                (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
                (Self::Integer(a), Self::Integer(b)) => a.cmp(b),
                (Self::Float(a), Self::Float(b)) => {
                    let a_norm = normalize_float(*a);
                    let b_norm = normalize_float(*b);
                    a_norm.to_bits().cmp(&b_norm.to_bits())
                }
                (Self::String(a), Self::String(b)) => a.cmp(b),
                _ => unreachable!(),
            },
            ord => ord,
        }
    }
}

impl std::hash::Hash for AtomValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.discriminant_idx().hash(state);
        match self {
            Self::Bool(b) => b.hash(state),
            Self::Integer(i) => i.hash(state),
            Self::Float(f) => normalize_float(*f).to_bits().hash(state),
            Self::String(s) => s.hash(state),
        }
    }
}

pub(crate) enum Value {
    Single(AtomValue),
    Set(BTreeSet<AtomValue>),
    Null,
}

impl Value {
    fn from_any_value(v: AnyValue<'_>) -> Self {
        match v {
            AnyValue::Null => Value::Null,
            AnyValue::List(series) => {
                let mut set = BTreeSet::new();
                for v in series.iter() {
                    assert!(set.insert(AtomValue::from_any_value(v)));
                }
                Value::Set(set)
            }
            v => Self::Single(AtomValue::from_any_value(v)),
        }
    }
}

pub(crate) struct Entity {
    fields: BTreeMap<ColumnId, Value>,
}

type ValueCreator<T> = Box<dyn Fn(&T) -> Result<AnyValue<'static>, TableError> + Send + Sync>;

struct SchemaTypeInfo<T> {
    data_type: DataType,
    value_creator: ValueCreator<T>,
}

fn atom_type_to_data_type(typ: &AtomType) -> Result<SchemaTypeInfo<AtomValue>, SchemaBuildError> {
    Ok(match typ {
        AtomType::Integer => SchemaTypeInfo {
            data_type: DataType::Int64,
            value_creator: Box::new(|v| {
                let AtomValue::Integer(n) = v else {
                    bail!("Expected number for integer column.")
                };
                Ok(AnyValue::Int64(*n))
            }),
        },
        AtomType::String => SchemaTypeInfo {
            data_type: DataType::String,
            value_creator: Box::new(|v| {
                let AtomValue::String(s) = v else {
                    bail!("Expected string for string column.")
                };
                Ok(AnyValue::StringOwned(PlSmallStr::from_str(s)))
            }),
        },
        AtomType::DateTime => SchemaTypeInfo {
            data_type: DataType::Datetime(TimeUnit::Milliseconds, Some(TimeZone::UTC)),
            value_creator: Box::new(|v| {
                let AtomValue::String(s) = v else {
                    bail!("Expected string in RFC 3338 format for datetime column.")
                };
                let datetime = chrono::DateTime::parse_from_rfc3339(s)
                    .raise()
                    .msg("field had invalid date format")?
                    .with_timezone(&chrono::Utc);
                Ok(AnyValue::DatetimeOwned(
                    datetime.timestamp_millis(),
                    TimeUnit::Milliseconds,
                    Some(Arc::new(TimeZone::UTC)),
                ))
            }),
        },
        AtomType::Bool => SchemaTypeInfo {
            data_type: DataType::Boolean,
            value_creator: Box::new(|v| {
                let AtomValue::Bool(b) = v else {
                    bail!("Expected boolean for boolean column.")
                };
                Ok(AnyValue::Boolean(*b))
            }),
        },
        AtomType::Enum(enum_schema) => {
            let mapping = FrozenCategories::new(enum_schema.values.iter().map(|s| &**s))
                .raise()
                .msg("Invalid enum spec")?;
            let cats = mapping.clone();
            let cat_mapping = mapping.mapping().clone();
            SchemaTypeInfo {
                data_type: DataType::Enum(mapping, cat_mapping),
                value_creator: Box::new(move |v| {
                    let AtomValue::String(s) = v else {
                        bail!("Expected string for enum column.")
                    };
                    let value = cats.mapping().insert_cat(s).raise_with(diag!(
                        || "Invalid enum value {:?}. Possible values are {:?}",
                        s,
                        cats.categories()
                    ))?;

                    Ok(AnyValue::EnumOwned(value, cats.mapping().clone()))
                }),
            }
        }
    })
}

fn to_value_creator<T, F>(f: F) -> ValueCreator<T>
where
    F: for<'a> Fn(&'a T) -> Result<AnyValue<'static>, TableError> + Send + Sync + 'static,
{
    let boxed: ValueCreator<T> = Box::new(f);
    boxed
}

fn column_kind_to_data_type(
    ty: &ColumnType,
    is_nullable: bool,
) -> Result<SchemaTypeInfo<Value>, SchemaBuildError> {
    Ok(match ty {
        ColumnType::Single(atom_type) => {
            let SchemaTypeInfo {
                data_type,
                value_creator,
            } = atom_type_to_data_type(atom_type)?;

            let value_creator = to_value_creator(move |v| match v {
                Value::Single(atom_value) => value_creator(atom_value),
                Value::Set(_values) => bail!("Expected single value for single column."),
                Value::Null => {
                    ensure!(is_nullable, "Null value found for non-nullable column");
                    Ok(AnyValue::Null)
                }
            });
            SchemaTypeInfo {
                data_type,
                value_creator,
            }
        }
        ColumnType::Set(col_type) => {
            let SchemaTypeInfo {
                data_type: elem_data_type,
                value_creator: elem_value_creator,
            } = atom_type_to_data_type(col_type)?;

            let list_data_type = DataType::List(Box::new(elem_data_type.clone()));
            let list_value_creator = to_value_creator({
                const EMPTY_SET: BTreeSet<AtomValue> = BTreeSet::new();
                move |v| {
                    let items = match v {
                        Value::Set(arr) => arr,
                        Value::Null if is_nullable => &EMPTY_SET,
                        _ => bail!("Expected array for set column."),
                    };

                    let mut builder = SeriesBuilder::new(elem_data_type.clone());
                    for item in items {
                        let new_value: AnyValue = elem_value_creator(item)?;
                        builder.push_any_value(new_value);
                    }
                    Ok(AnyValue::List(
                        builder.freeze(PlSmallStr::from_static("set_entries")),
                    ))
                }
            });
            SchemaTypeInfo {
                data_type: list_data_type,
                value_creator: list_value_creator,
            }
        }
    })
}

fn get_singleton_u32(df: &DataFrame) -> Result<u32, AnyDiag> {
    in_err_context(|| {
        ensure!(
            df.columns().len() == 1,
            "Dataframe should have single column"
        );
        let v = Vec::from(df[0].u32()?);
        ensure!(v.len() == 1, "Dataframe should have single row.");
        let Some(i) = v[0] else {
            bail!("Dataframe should not be null")
        };
        Ok(i)
    })
    .reraise()
}

fn check_unique(
    df: &DataFrame,
    cols: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<(), AnyDiag> {
    let lazy_frame = df.clone().lazy();
    in_err_context(|| {
        let unique_count = get_singleton_u32(
            &lazy_frame
                .clone()
                .select(
                    cols.into_iter()
                        .map(|s| col(PlSmallStr::from_str(s.as_ref())))
                        .collect::<Vec<_>>(),
                )
                .unique(None, UniqueKeepStrategy::Any)
                .count()
                .collect()?,
        )?;

        let complete_count = get_singleton_u32(&lazy_frame.count().collect()?)?;
        if unique_count != complete_count {
            //
        }
        Ok(())
    })
    .map_raise_err(diag!(|e| "Unique constraint failed: {e}"))?;
    Ok(())
}

fn create_data_frame(
    schema: &InnerTableSchema,
    items: impl IntoIterator<Item = impl Borrow<Entity>>,
) -> Result<DataFrame, TableError> {
    struct SeriesBuilderInfo<'a> {
        schema: &'a ColumnSchema,
        column_builder: SeriesBuilder,
    }

    let mut series_builder_map = BTreeMap::<ColumnId, SeriesBuilderInfo>::new();

    for (col_id, col_schema) in &schema.columns {
        let btree_map::Entry::Vacant(vac) = series_builder_map.entry(col_id.clone()) else {
            unreachable!("by construction");
        };
        vac.insert(SeriesBuilderInfo {
            schema: col_schema,
            column_builder: SeriesBuilder::new(col_schema.polars_type.clone()),
        });
    }

    for entity in items {
        let entity = entity.borrow();
        for (col_id, info) in &mut series_builder_map {
            let json_value = entity.fields.get(col_id.as_str()).unwrap_or(&Value::Null);
            let any_value = (info.schema.value_creator)(json_value)?;

            info.column_builder.push_any_value(any_value);
        }

        for name in entity.fields.keys() {
            ensure!(
                series_builder_map.contains_key(name.as_str()),
                "Entity specified column that does not exist."
            );
        }
    }

    let frame = DataFrame::new_infer_height(
        series_builder_map
            .into_values()
            .map(|info| {
                info.column_builder
                    .freeze(PlSmallStr::from_str(info.schema.name.as_str()))
                    .into()
            })
            .collect(),
    )
    .raise()
    .msg("Failed to combine columns into dataframe")?;

    // Check provided constraints against data frame.
    check_unique(&frame, &schema.primary_key.fields)?;

    for constraint in &schema.constraints {
        match constraint {
            Constraint::Unique(unique_constraint) => {
                check_unique(&frame, &unique_constraint.fields)?;
            }
        }
    }

    Ok(frame)
}

define_error! {
    pub(crate) struct SchemaBuildError;
}

fn make_unique_constraint(
    columns: &BTreeMap<ColumnId, ColumnSchema>,
    unique_columns: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<UniqueConstraint, SchemaBuildError> {
    let mut unique_key_set = BTreeSet::new();
    for col in unique_columns {
        let col = col.as_ref();
        let Some((col_id, _column)) = columns.get_key_value(col) else {
            bail!("Unknown key: {col:?}");
        };

        if !unique_key_set.insert(col_id.clone()) {
            bail!("Duplicate key: {}", col);
        }
    }
    Ok(UniqueConstraint {
        fields: unique_key_set,
    })
}

pub(crate) struct TableSchemaBuilder {
    columns: BTreeMap<ColumnId, ColumnSchema>,
    unique_constraints: BTreeSet<UniqueConstraint>,
}

impl TableSchemaBuilder {
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn add_column(
        &mut self,
        name: impl AsRef<str>,
        ty: impl Into<ColumnType>,
    ) -> Result<ColumnSchemaBuilder<'_>, SchemaBuildError> {
        let col_id = ColumnId(PlSmallStr::from_str(name.as_ref()));
        Ok(ColumnSchemaBuilder {
            parent: self,
            name: col_id,
            description: None,
            value_type: ty.into(),
            nullable: false,
        })
    }

    fn with_unique_constraint(
        mut self,
        unique_columns: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, SchemaBuildError> {
        let unique_constraint = make_unique_constraint(&self.columns, unique_columns)?;
        if !self.unique_constraints.insert(unique_constraint.clone()) {
            bail!("Redundant unique constraint: {unique_constraint:?}",);
        }
        Ok(self)
    }

    fn build(
        self,
        primary_key: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<TableSchema, SchemaBuildError> {
        let unique_constraint = make_unique_constraint(&self.columns, primary_key)?;
        //column_kind_to_data_type(&column.value_type, column.nullable)?;
        Ok(TableSchema(Arc::new(InnerTableSchema {
            columns: self.columns,
            primary_key: unique_constraint,
            constraints: self
                .unique_constraints
                .into_iter()
                .map(Constraint::Unique)
                .collect(),
        })))
    }
}

pub(crate) struct ColumnSchemaBuilder<'a> {
    parent: &'a mut TableSchemaBuilder,
    name: ColumnId,
    description: Option<String>,
    value_type: ColumnType,
    nullable: bool,
}

impl ColumnSchemaBuilder<'_> {
    pub(crate) fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
    pub(crate) fn set_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    pub(crate) fn build(self) -> Result<ColumnId, SchemaBuildError> {
        let col_id = self.name.clone();
        match self.parent.columns.entry(col_id.clone()) {
            btree_map::Entry::Vacant(vac) => {
                let type_info = column_kind_to_data_type(&self.value_type, self.nullable)?;
                vac.insert(ColumnSchema {
                    name: self.name,
                    description: self.description,
                    value_type: self.value_type,
                    nullable: self.nullable,
                    polars_type: type_info.data_type,
                    value_creator: type_info.value_creator,
                });
            }

            btree_map::Entry::Occupied(occupied_entry) => {
                bail!("Duplicate column name {:?}", occupied_entry.key());
            }
        }
        Ok(col_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ColumnId(PlSmallStr);

impl ColumnId {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ColumnId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ColumnId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

trait ColumnWriter {
    fn write_next(&mut self, entity: &mut Entity) -> bool;
}

struct ColumnWriterImpl<I, F>(I, F);

impl<I, F> ColumnWriter for ColumnWriterImpl<I, F>
where
    I: Iterator,
    F: Fn(&mut Entity, I::Item),
{
    fn write_next(&mut self, entity: &mut Entity) -> bool {
        if let Some(next_val) = self.0.next() {
            (self.1)(entity, next_val);
            true
        } else {
            false
        }
    }
}

pub(crate) struct EntityIter<'a> {
    writers: Vec<Box<dyn ColumnWriter + 'a>>,
}

impl Iterator for EntityIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let mut entity = Entity {
            fields: BTreeMap::new(),
        };
        let writers = &mut *self.writers;
        let Some((first_writer, rest_writers)) = writers.split_first_mut() else {
            unreachable!("table needs at least one column.");
        };

        if first_writer.write_next(&mut entity) {
            for writer in rest_writers {
                assert!(writer.write_next(&mut entity));
            }
            Some(entity)
        } else {
            for writer in rest_writers {
                assert!(!writer.write_next(&mut entity));
            }
            None
        }
    }
}

pub(crate) struct Table {
    schema: TableSchema,
    frame: DataFrame,
}

impl Table {
    pub(crate) fn to_entities(&self) -> EntityIter<'_> {
        let frame_columns = self.frame.columns();
        let mut writers: Vec<Box<dyn ColumnWriter + '_>> = Vec::with_capacity(frame_columns.len());
        for col in self.frame.columns() {
            let col_schema = self
                .schema
                .0
                .columns
                .get(col.name().as_str())
                .expect("Frame and schema names should match.");
            let iter = col.as_materialized_series().iter();
            let name = col_schema.name.clone();
            let col_writer = ColumnWriterImpl(iter, move |entity: &mut Entity, value: AnyValue| {
                entity
                    .fields
                    .insert(name.clone(), Value::from_any_value(value));
            });

            writers.push(Box::new(col_writer));
        }

        EntityIter { writers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! entity {
        ( $($key:ident: $value:expr),* $(,)? ) => {
            {
                Entity {
                    fields: [
                        $( (ColumnId(PlSmallStr::from_static(stringify!($key))), $value)),*,
                    ].into_iter().collect()
                }
            }
        };

    }

    #[test]
    fn test_basic_key_entities() -> anyhow::Result<()> {
        let entities = vec![
            entity! {
                key: Value::Single(AtomValue::Integer(1)),
            },
            entity! {
                key: Value::Single(AtomValue::Integer(2)),
            },
        ];
        let mut schema_builder = TableSchema::builder();
        let key_column = schema_builder
            .add_column("key", AtomType::Integer)?
            .build()?;
        let schema = schema_builder.build([key_column])?;

        let table = schema.create_table(&entities)?;
        let entities = table.to_entities().collect::<Vec<_>>();
        assert_eq!(entities.len(), 2);
        Ok(())
    }
}
