use std::{
    any::Any,
    fmt::{self, Debug, Display},
};

use crate::Kind;

/// An alias-like trait for types that are Display, Debug, Send, Sync and 'static.
///
/// Note that types that already implement [`std::error::Error`] automatically
/// implement all of these traits, so automatically implement [`ErrLike`].
pub trait Reportable: Display + Debug + Send + Sync + 'static {}

impl<Obj> Reportable for Obj where Obj: Display + Debug + Send + Sync + 'static {}

trait DynReportable: Debug + Send + Sync + 'static {
    fn as_reportable(&self) -> &dyn Reportable;
    fn as_any(&self) -> Option<&dyn Any>;
    fn into_any_box(self: Box<Self>) -> Option<Box<dyn Any>>;
}

#[derive(Debug)]
struct KindWrapper<K>(K);

impl<K> DynReportable for KindWrapper<K>
where
    K: Kind + Reportable,
{
    fn as_reportable(&self) -> &dyn Reportable {
        &self.0
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(&self.0)
    }

    fn into_any_box(self: Box<Self>) -> Option<Box<dyn Any>> {
        Some(Box::new(self.0))
    }
}

#[derive(Debug)]
struct SplitReportable<K, R> {
    value: K,
    reportable: R,
}

impl<K, R> DynReportable for SplitReportable<K, R>
where
    K: Kind,
    R: Reportable,
{
    fn as_reportable(&self) -> &dyn Reportable {
        &self.reportable
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(&self.value)
    }

    fn into_any_box(self: Box<Self>) -> Option<Box<dyn Any>> {
        Some(Box::new(self.value))
    }
}

#[derive(Debug)]
struct OnlyReportable<R> {
    reportable: R,
}

impl<R> DynReportable for OnlyReportable<R>
where
    R: Reportable,
{
    fn as_reportable(&self) -> &dyn Reportable {
        &self.reportable
    }

    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    fn into_any_box(self: Box<Self>) -> Option<Box<dyn Any>> {
        None
    }
}

pub(crate) struct ReportableHandle(Box<dyn DynReportable>);

impl ReportableHandle {
    pub(crate) fn new<K>(err_like: K) -> Self
    where
        K: Kind + Reportable,
    {
        Self(Box::new(KindWrapper(err_like)))
    }

    pub(crate) fn from_split<K, R>(value: K, reportable: R) -> Self
    where
        K: Kind,
        R: Reportable,
    {
        Self(Box::new(SplitReportable { value, reportable }))
    }

    pub(crate) fn from_report_only<R>(reportable: R) -> Self
    where
        R: Reportable,
    {
        Self(Box::new(OnlyReportable { reportable }))
    }

    pub(crate) fn as_ref(&self) -> &dyn Reportable {
        self.0.as_reportable()
    }

    pub(crate) fn downcast_ref<K>(&self) -> Option<&K>
    where
        K: Kind,
    {
        self.0.as_any()?.downcast_ref()
    }

    pub(crate) fn downcast<K>(self) -> Option<K>
    where
        K: Kind,
    {
        self.0.into_any_box()?.downcast::<K>().map(|v| *v).ok()
    }
}

impl Display for ReportableHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self.0.as_reportable(), f)
    }
}

impl Debug for ReportableHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.0.as_reportable(), f)
    }
}
