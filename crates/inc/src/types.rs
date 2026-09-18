use std::{any::Any, sync::Arc};

pub(crate) type AnyValue = dyn Any + Send + Sync + 'static;

pub(crate) type AnyArc = Arc<AnyValue>;
