use std::{
    any::Any,
    fmt::{self, Debug, Display},
    sync::{Arc, Weak},
};

use crate::Kind;

/// An alias-like trait for types that are Display, Debug, Send, Sync and 'static.
///
/// Note that types that already implement [`std::error::Error`] automatically
/// implement all of these traits, so automatically implement [`Reportable`].
pub trait Reportable: Display + Debug + Send + Sync + 'static {}

impl<Obj> Reportable for Obj where Obj: Display + Debug + Send + Sync + 'static {}

// Some internal object traits for managing the Kind + Reportable, and the
// split to a strong Kind handle, and a weak Reportable handle.

trait ReportableKind: Kind + DynReportable {
    fn split(self: Arc<Self>) -> (Arc<dyn Any + Send + Sync>, Weak<dyn DynReportable>);
}

impl<Obj> ReportableKind for Obj
where
    Obj: Kind + DynReportable,
{
    fn split(self: Arc<Self>) -> (Arc<dyn Any + Send + Sync>, Weak<dyn DynReportable>) {
        let kind = Arc::downgrade(&self);
        (self, kind)
    }
}

// To be able to upcast to Any, which is needed in many cases, we define a
// dyn-compatible object trait for Reportable + Any.

trait DynReportable: Reportable + Any {}

impl<Obj> DynReportable for Obj where Obj: Reportable + Any {}

struct AppendedReportable(Vec<MaybeWeak<dyn DynReportable>>);

impl Display for AppendedReportable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for rep in &self.0 {
            if first {
                first = false;
            } else {
                f.write_str(": ")?;
            }
            rep.with(|rep| match rep {
                Some(rep) => Display::fmt(rep, f),
                None => f.write_str("<removed>"),
            })?;
        }
        Ok(())
    }
}

impl Debug for AppendedReportable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppendReportable([")?;
        let mut first = true;
        for rep in &self.0 {
            if first {
                first = false;
            } else {
                f.write_str(", ")?;
            }
            rep.with(|rep| match rep {
                Some(rep) => Debug::fmt(rep, f),
                None => f.write_str("<removed>"),
            })?;
        }
        f.write_str("])")?;
        Ok(())
    }
}

fn extract_reportables(dyn_rep: MaybeWeak<dyn DynReportable>) -> Vec<MaybeWeak<dyn DynReportable>> {
    match dyn_rep {
        x @ MaybeWeak::Weak(_) => vec![x],
        MaybeWeak::Strong(strong) => {
            let any_ref: &dyn Any = &*strong;
            if any_ref.is::<AppendedReportable>() {
                let cast_arc = Arc::downcast::<AppendedReportable>(strong).unwrap();
                match Arc::try_unwrap(cast_arc) {
                    Ok(appended) => appended.0,
                    Err(strong) => strong.0.clone(),
                }
            } else {
                vec![MaybeWeak::Strong(strong)]
            }
        }
    }
}

fn extract_all_reportables(
    reps: impl IntoIterator<Item = MaybeWeak<dyn DynReportable>>,
) -> Vec<MaybeWeak<dyn DynReportable>> {
    let mut reps_iter = reps.into_iter().map(extract_reportables);

    // Try to reuse any memory if available by extracting first element.
    let Some(mut reps) = reps_iter.next() else {
        return vec![];
    };

    reps.extend(reps_iter.flatten());
    reps
}

fn concat_reportables(
    reps: impl IntoIterator<Item = MaybeWeak<dyn DynReportable>>,
) -> Arc<dyn DynReportable> {
    let all_reps = extract_all_reportables(reps);
    assert!(all_reps.len() >= 2, "Must concat at least two entities");
    Arc::new(AppendedReportable(all_reps))
}

enum Contents {
    ReportableKind(Arc<dyn ReportableKind>),
    SplitReportable(Arc<dyn Any + Send + Sync>, Arc<dyn DynReportable>),
    OnlyReportable(Arc<dyn DynReportable>),
}

enum MaybeWeak<T>
where
    T: ?Sized,
{
    Weak(Weak<T>),
    Strong(Arc<T>),
}

impl<T> MaybeWeak<T>
where
    T: ?Sized,
{
    fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Option<&T>) -> R,
    {
        match self {
            MaybeWeak::Weak(weak) => match weak.upgrade() {
                Some(strong) => f(Some(&*strong)),
                None => f(None),
            },
            MaybeWeak::Strong(strong) => f(Some(&**strong)),
        }
    }
}

impl<T> Clone for MaybeWeak<T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        match self {
            Self::Weak(arg0) => Self::Weak(Clone::clone(arg0)),
            Self::Strong(arg0) => Self::Strong(Clone::clone(arg0)),
        }
    }
}

pub(crate) struct ReportableHandle(Contents);

impl ReportableHandle {
    pub(crate) fn new<K>(err_like: K) -> Self
    where
        K: Kind + Reportable,
    {
        Self(Contents::ReportableKind(Arc::new(err_like)))
    }

    pub(crate) fn from_split<K, R>(value: K, reportable: R) -> Self
    where
        K: Kind,
        R: Reportable,
    {
        Self(Contents::SplitReportable(
            Arc::new(value),
            Arc::new(reportable),
        ))
    }

    pub(crate) fn from_report_only<R>(reportable: R) -> Self
    where
        R: Reportable,
    {
        Self(Contents::OnlyReportable(Arc::new(reportable)))
    }

    pub(crate) fn as_reportable(&self) -> &dyn Reportable {
        match &self.0 {
            Contents::ReportableKind(k) => k.as_ref(),
            Contents::SplitReportable(_, r) | Contents::OnlyReportable(r) => r.as_ref(),
        }
    }

    pub(crate) fn downcast_ref<K>(&self) -> Option<&K>
    where
        K: Kind,
    {
        let any_ref: &(dyn Any + Send + Sync) = match &self.0 {
            Contents::ReportableKind(reportable_kind) => &**reportable_kind,
            Contents::SplitReportable(any, _) => &**any,
            Contents::OnlyReportable(_) => return None,
        };
        any_ref.downcast_ref::<K>()
    }

    pub(crate) fn downcast<K>(self) -> Option<K>
    where
        K: Kind,
    {
        let any_box = match self.0 {
            Contents::ReportableKind(reportable_kind) => reportable_kind.split().0,
            Contents::SplitReportable(any, _) => any,
            Contents::OnlyReportable(_) => return None,
        };
        Arc::downcast::<K>(any_box).ok().and_then(Arc::into_inner)
    }

    pub(crate) fn append_reportable(self, rep: impl Reportable) -> Self {
        let rep: MaybeWeak<dyn DynReportable> = MaybeWeak::Strong(Arc::new(rep));
        let contents = match self.0 {
            Contents::ReportableKind(reportable_kind) => {
                let (kind, reportable) = reportable_kind.split();
                Contents::SplitReportable(
                    kind,
                    concat_reportables([MaybeWeak::Weak(reportable), rep]),
                )
            }
            Contents::SplitReportable(kind, reportable) => Contents::SplitReportable(
                kind,
                concat_reportables([MaybeWeak::Strong(reportable), rep]),
            ),
            Contents::OnlyReportable(reportable) => {
                Contents::OnlyReportable(concat_reportables([MaybeWeak::Strong(reportable), rep]))
            }
        };
        Self(contents)
    }

    pub(crate) fn clone_weak(&self) -> WeakReportableHandle {
        WeakReportableHandle(match &self.0 {
            Contents::ReportableKind(reportable_kind) => {
                let weak_rep = Arc::downgrade(reportable_kind);
                MaybeWeak::Weak(weak_rep as Weak<dyn DynReportable>)
            }
            Contents::SplitReportable(_, r) | Contents::OnlyReportable(r) => {
                MaybeWeak::Strong(r.clone())
            }
        })
    }
}

impl Display for ReportableHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self.as_reportable(), f)
    }
}

impl Debug for ReportableHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(self.as_reportable(), f)
    }
}

/// A handle to a reportable that while live will display and debug the
/// same way as the original reportable.
#[derive(Clone)]
pub(crate) struct WeakReportableHandle(MaybeWeak<dyn DynReportable>);

impl WeakReportableHandle {
    pub(crate) fn new_dangling() -> Self {
        Self(MaybeWeak::Weak(Weak::<std::convert::Infallible>::new()))
    }
}

impl Display for WeakReportableHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.with(|rep| match rep {
            Some(rep) => Display::fmt(rep, f),
            None => f.write_str("<removed>"),
        })
    }
}

impl Debug for WeakReportableHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.with(|rep| match rep {
            Some(rep) => Debug::fmt(rep, f),
            None => f.write_str("<removed>"),
        })
    }
}
