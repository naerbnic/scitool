use std::collections::{BTreeMap, btree_map};

use polars::{prelude::*, series::builder::SeriesBuilder};
use scidev_errors::{AnyDiag, bail, define_error, diag, ensure, in_err_context, prelude::*};
use serde_json::Value as JsonValue;

define_error! {
    pub struct TableError;
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) struct EnumSchema {
    values: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) enum ColumnType {
    Integer,
    String,
    DateTime,
    Bool,
    Enum(EnumSchema),
}

#[derive(Clone, Debug, serde::Deserialize)]
pub(crate) enum ColumnKind {
    Atom(ColumnType),
    Set(ColumnType),
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
    value_type: ColumnKind,

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

type ValueCreator = Box<dyn Fn(&JsonValue) -> Result<AnyValue<'static>, TableError>>;

struct SchemaTypeInfo {
    data_type: DataType,
    value_creator: ValueCreator,
}

fn column_type_to_data_type(typ: &ColumnType) -> Result<SchemaTypeInfo, TableError> {
    Ok(match typ {
        ColumnType::Integer => SchemaTypeInfo {
            data_type: DataType::Int32,
            value_creator: Box::new(|v| {
                let JsonValue::Number(n) = v else {
                    bail!("Expected number for integer column.")
                };

                let value = n
                    .as_i64()
                    .and_then(|i| i.try_into().ok())
                    .ok_or_else(|| diag!("Number out of bounds: {n:?}"))?;
                Ok(AnyValue::Int32(value))
            }),
        },
        ColumnType::String => SchemaTypeInfo {
            data_type: DataType::String,
            value_creator: Box::new(|v| {
                let JsonValue::String(s) = v else {
                    bail!("Expected string for string column.")
                };
                Ok(AnyValue::StringOwned(PlSmallStr::from_str(s)))
            }),
        },
        ColumnType::DateTime => SchemaTypeInfo {
            data_type: DataType::Datetime(TimeUnit::Milliseconds, Some(TimeZone::UTC)),
            value_creator: Box::new(|v| {
                let JsonValue::String(s) = v else {
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
        ColumnType::Bool => SchemaTypeInfo {
            data_type: DataType::Boolean,
            value_creator: Box::new(|v| {
                let JsonValue::Bool(b) = v else {
                    bail!("Expected boolean for boolean column.")
                };
                Ok(AnyValue::Boolean(*b))
            }),
        },
        ColumnType::Enum(enum_schema) => {
            let mapping = FrozenCategories::new(enum_schema.values.iter().map(|s| &**s))
                .raise()
                .msg("Invalid enum spec")?;
            let cats = mapping.clone();
            let cat_mapping = mapping.mapping().clone();
            SchemaTypeInfo {
                data_type: DataType::Enum(mapping, cat_mapping),
                value_creator: Box::new(move |v| {
                    let JsonValue::String(s) = v else {
                        bail!("Expected string for enum column.")
                    };
                    let value = cats.mapping().insert_cat(&s).raise_with(diag!(
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

fn to_value_creator<F>(f: F) -> ValueCreator
where
    F: for<'a> Fn(&'a JsonValue) -> Result<AnyValue<'static>, TableError> + 'static,
{
    let boxed: ValueCreator = Box::new(f);
    boxed
}

fn column_kind_to_data_type(
    kind: &ColumnKind,
    is_nullable: bool,
) -> Result<SchemaTypeInfo, TableError> {
    Ok(match kind {
        ColumnKind::Atom(col_type) => {
            let SchemaTypeInfo {
                data_type,
                value_creator,
            } = column_type_to_data_type(col_type)?;

            let value_creator = if is_nullable {
                to_value_creator(move |v| {
                    if let JsonValue::Null = v {
                        Ok(AnyValue::Null)
                    } else {
                        value_creator(&v)
                    }
                })
            } else {
                value_creator
            };

            if is_nullable {
                SchemaTypeInfo {
                    data_type,
                    value_creator: Box::new(move |v| {
                        if let JsonValue::Null = v {
                            Ok(AnyValue::Null)
                        } else {
                            value_creator(v)
                        }
                    }),
                }
            } else {
                SchemaTypeInfo {
                    data_type,
                    value_creator,
                }
            }
        }
        ColumnKind::Set(col_type) => {
            let SchemaTypeInfo {
                data_type: elem_data_type,
                value_creator: elem_value_creator,
            } = column_type_to_data_type(col_type)?;

            let list_data_type = DataType::List(Box::new(elem_data_type.clone()));
            let list_value_creator = to_value_creator({
                move |v| {
                    let items = match v {
                        JsonValue::Array(arr) => &**arr,
                        JsonValue::Null if is_nullable => &[],
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

pub(crate) fn create_data_frame(
    schema: &TableSchema,
    items: &[serde_json::Value],
) -> Result<DataFrame, TableError> {
    struct SchemaInfo<'a> {
        name: &'a str,
        column_builder: SeriesBuilder,
        value_creator: ValueCreator,
    }

    let mut col_schema_map: BTreeMap<PlSmallStr, SchemaInfo> = BTreeMap::new();

    for col in &schema.columns {
        let name = PlSmallStr::from_str(&col.name);
        let btree_map::Entry::Vacant(vac) = col_schema_map.entry(name.clone()) else {
            bail!("Duplicate column name {:?}", col.name);
        };

        let SchemaTypeInfo {
            data_type,
            value_creator,
        } = column_kind_to_data_type(&col.value_type, col.nullable)?;
        vac.insert(SchemaInfo {
            name: &col.name,
            column_builder: SeriesBuilder::new(data_type),
            value_creator,
        });
    }

    for entity in items {
        let serde_json::Value::Object(fields) = &entity else {
            bail!("Got non-object as an entity.");
        };
        for (name, info) in &mut col_schema_map {
            let json_value = fields.get(name.as_str()).unwrap_or(&JsonValue::Null);
            let any_value = (info.value_creator)(json_value)?;

            info.column_builder.push_any_value(any_value);
        }

        for name in fields.keys() {
            ensure!(
                col_schema_map.contains_key(name.as_str()),
                "Entity specified column that does not exist."
            );
        }
    }

    let frame = DataFrame::new_infer_height(
        col_schema_map
            .into_values()
            .map(|info| {
                info.column_builder
                    .freeze(PlSmallStr::from_str(info.name))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_basic_key_entities() {
        let entities = vec![
            json!({
                "key": 1,
            }),
            json!({
                "key": 2,
            }),
        ];
        let schema = TableSchema {
            columns: vec![ColumnSchema {
                name: "key".to_string(),
                description: None,
                value_type: ColumnKind::Atom(ColumnType::Integer),
                nullable: false,
            }],
            primary_key: UniqueConstraint {
                fields: vec!["key".to_string()],
            },
            constraints: vec![],
        };
        let frame = create_data_frame(&schema, &entities).unwrap();
    }
}
