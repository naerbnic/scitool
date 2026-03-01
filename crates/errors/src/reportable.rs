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
struct ReportableWraper<T>(T);

impl<T> DynReportable for ReportableWraper<T>
where
    T: Kind + Reportable,
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
struct SplitReportable<T, E> {
    value: T,
    reportable: E,
}

impl<T, E> DynReportable for SplitReportable<T, E>
where
    T: Kind,
    E: Reportable,
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
struct OnlyReportable<E> {
    reportable: E,
}

impl<E> DynReportable for OnlyReportable<E>
where
    E: Reportable,
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

pub(crate) struct BoxedErrLike(Box<dyn DynReportable>);

impl BoxedErrLike {
    pub(crate) fn new<E>(err_like: E) -> Self
    where
        E: Kind + Reportable,
    {
        Self(Box::new(ReportableWraper(err_like)))
    }

    pub(crate) fn from_split<T, E>(value: T, reportable: E) -> Self
    where
        T: Kind,
        E: Reportable,
    {
        Self(Box::new(SplitReportable { value, reportable }))
    }

    pub(crate) fn from_report_only<E>(reportable: E) -> Self
    where
        E: Reportable,
    {
        Self(Box::new(OnlyReportable { reportable }))
    }

    pub(crate) fn as_ref(&self) -> &dyn Reportable {
        self.0.as_reportable()
    }

    pub(crate) fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Kind,
    {
        self.0.as_any()?.downcast_ref()
    }

    pub(crate) fn downcast<T>(self) -> Option<T>
    where
        T: Kind,
    {
        self.0.into_any_box()?.downcast::<T>().map(|v| *v).ok()
    }
}

impl Display for BoxedErrLike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self.0.as_reportable(), f)
    }
}

impl Debug for BoxedErrLike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.0.as_reportable(), f)
    }
}
