#[derive(Debug, thiserror::Error)]
#[error("Object data has unexpected padding bytes")]
pub(super) struct BadObjectPadding;

#[derive(Debug, thiserror::Error)]
#[error(
    "Class has script but number of properties does not equal number of fields: {num_properties} properties, {num_fields} fields"
)]
pub(super) struct PropertyMismatch {
    pub num_properties: usize,
    pub num_fields: usize,
}
