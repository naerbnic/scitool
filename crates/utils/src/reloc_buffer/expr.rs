//! A relocation entry may be the result of more than one symbol reference.
//! We allow the relocation values to be simple expressions, which are
//! partially evaluated at the time of the relocation.

use crate::numbers::bit_convert::{NumConvert, WidenTo};

use super::LocalResolver;

/// A value for a symbol, before it has been fully resolved.
#[derive(Clone, Copy, Debug)]
struct IntermediateValue(Option<(i64, i64)>);

impl IntermediateValue {
    pub fn new_base_relative(offset: i64) -> Self {
        IntermediateValue(Some((offset, 1)))
    }

    pub fn new_exact(offset: i64) -> Self {
        IntermediateValue(Some((offset, 0)))
    }

    pub fn new_unknown() -> Self {
        IntermediateValue(None)
    }

    pub fn add(self, other: Self) -> Self {
        match (self.0, other.0) {
            (Some((a, b)), Some((c, d))) => IntermediateValue(Some((a + c, b + d))),
            _ => IntermediateValue(None),
        }
    }

    pub fn sub(self, other: Self) -> Self {
        match (self.0, other.0) {
            (Some((a, b)), Some((c, d))) => IntermediateValue(Some((a - c, b - d))),
            _ => IntermediateValue(None),
        }
    }

    pub fn scalar_multiply(self, other: i64) -> Self {
        match self.0 {
            Some((a, b)) => IntermediateValue(Some((a * other, b * other))),
            None => IntermediateValue(None),
        }
    }

    pub fn exact_value(&self) -> Option<i64> {
        let (offset, coefficient) = self.0?;
        if coefficient == 0 {
            Some(offset)
        } else {
            None
        }
    }
}

/// A primitive expression value that yields a single value.
#[derive(Clone, Debug)]
enum LeafValue<Ext, Sym> {
    /// The current address of the relocation entry.
    CurrentAddress,

    /// An immediate value.
    Immediate(i64),

    /// The location value of the symbol within this relocatable buffer.
    LocalSymbol(Sym),

    /// A value that comes from an external source, and may not be an
    /// address.
    ExternalValue(Ext),
}

impl<Ext, Sym> LeafValue<Ext, Sym> {
    fn partial_eval<R>(&self, resolver: &R, current_address: usize) -> Option<IntermediateValue>
    where
        R: LocalResolver<Sym>,
    {
        let result = match self {
            LeafValue::CurrentAddress => {
                IntermediateValue::new_base_relative(current_address.convert_num_to().unwrap())
            }
            LeafValue::Immediate(value) => IntermediateValue::new_exact(*value),
            LeafValue::LocalSymbol(sym) => {
                IntermediateValue::new_base_relative(resolver.resolve_local_symbol(sym)?)
            }
            LeafValue::ExternalValue(_) => IntermediateValue::new_unknown(),
        };
        Some(result)
    }

    pub fn full_eval<R>(&self, resolver: &R, current_address: usize) -> anyhow::Result<i64>
    where
        Sym: std::fmt::Debug,
        R: super::FullResolver<Ext, Sym>,
    {
        match self {
            LeafValue::CurrentAddress => current_address.convert_num_to(),
            LeafValue::Immediate(value) => Ok(*value),
            LeafValue::LocalSymbol(sym) => resolver
                .resolve_local_symbol(sym)
                .ok_or_else(|| anyhow::anyhow!("failed to resolve local symbol {:?}", sym)),
            LeafValue::ExternalValue(value) => resolver.resolve(value),
        }
    }

    pub fn exact_value(&self) -> Option<i64> {
        match self {
            LeafValue::Immediate(value) => Some(*value),
            _ => None,
        }
    }
    pub fn filter_map_local<F, T>(self, mut body: F) -> anyhow::Result<LeafValue<Ext, T>>
    where
        F: FnMut(Sym) -> Option<T>,
        T: Clone,
    {
        Ok(match self {
            LeafValue::LocalSymbol(sym) => {
                if let Some(new_sym) = body(sym) {
                    LeafValue::LocalSymbol(new_sym)
                } else {
                    return Err(anyhow::anyhow!("failed to map local symbol"));
                }
            }
            LeafValue::CurrentAddress => LeafValue::CurrentAddress,
            LeafValue::Immediate(value) => LeafValue::Immediate(value),
            LeafValue::ExternalValue(value) => LeafValue::ExternalValue(value),
        })
    }
}

impl<Ext, Sym> LeafValue<Ext, Sym>
where
    Sym: Clone,
    Ext: Clone,
{
    pub fn map_external<Ext2>(&self, f: &impl Fn(&Ext) -> Ext2) -> LeafValue<Ext2, Sym> {
        match self {
            LeafValue::CurrentAddress => LeafValue::CurrentAddress,
            LeafValue::Immediate(value) => LeafValue::Immediate(*value),
            LeafValue::LocalSymbol(sym) => LeafValue::LocalSymbol(sym.clone()),
            LeafValue::ExternalValue(value) => LeafValue::ExternalValue(f(value)),
        }
    }

    pub fn map_local<Sym2>(&self, f: &impl Fn(&Sym) -> Sym2) -> LeafValue<Ext, Sym2> {
        match self {
            LeafValue::CurrentAddress => LeafValue::CurrentAddress,
            LeafValue::Immediate(value) => LeafValue::Immediate(*value),
            LeafValue::LocalSymbol(sym) => LeafValue::LocalSymbol(f(sym)),
            LeafValue::ExternalValue(value) => LeafValue::ExternalValue(value.clone()),
        }
    }
}

#[derive(Clone, Debug)]
enum ExprInner<Ext, Sym> {
    Value(LeafValue<Ext, Sym>),
    Difference(Box<Expr<Ext, Sym>>, Box<Expr<Ext, Sym>>),
    Sum(Box<Expr<Ext, Sym>>, Box<Expr<Ext, Sym>>),
    ScalarProduct(i64, Box<Expr<Ext, Sym>>),
}

#[derive(Clone, Debug)]
pub struct Expr<Ext, Sym>(ExprInner<Ext, Sym>);

impl<Ext, Sym> Expr<Ext, Sym>
where
    Ext: Clone,
    Sym: Clone,
{
    pub fn new_local(symbol: Sym) -> Self {
        Expr(ExprInner::Value(LeafValue::LocalSymbol(symbol)))
    }

    pub fn new_external(value: Ext) -> Self {
        Expr(ExprInner::Value(LeafValue::ExternalValue(value)))
    }

    pub fn new_literal(value: i64) -> Self {
        Expr(ExprInner::Value(LeafValue::Immediate(value)))
    }

    pub fn new_current_address() -> Self {
        Expr(ExprInner::Value(LeafValue::CurrentAddress))
    }

    pub fn new_add(a: Self, b: Self) -> Self {
        Expr(ExprInner::Sum(Box::new(a), Box::new(b)))
    }

    pub fn new_subtract(a: Self, b: Self) -> Self {
        Expr(ExprInner::Difference(Box::new(a), Box::new(b)))
    }

    pub fn new_scalar_product(coeff: i64, expr: Self) -> Self {
        Expr(ExprInner::ScalarProduct(coeff, Box::new(expr)))
    }

    pub fn local_symbols(&self) -> impl Iterator<Item = &Sym> {
        match &self.0 {
            ExprInner::Value(LeafValue::LocalSymbol(sym)) => {
                Box::new(std::iter::once(sym)) as Box<dyn Iterator<Item = &Sym>>
            }
            ExprInner::Value(_) => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = &Sym>>,
            ExprInner::Difference(a, b) | ExprInner::Sum(a, b) => {
                Box::new(a.local_symbols().chain(b.local_symbols()))
                    as Box<dyn Iterator<Item = &Sym>>
            }
            ExprInner::ScalarProduct(_, a) => a.local_symbols(),
        }
    }

    pub fn map_external<Ext2>(&self, f: &impl Fn(&Ext) -> Ext2) -> Expr<Ext2, Sym> {
        Expr(match &self.0 {
            ExprInner::Value(v) => ExprInner::Value(v.map_external(f)),
            ExprInner::Difference(a, b) => {
                ExprInner::Difference(Box::new(a.map_external(&f)), Box::new(b.map_external(&f)))
            }
            ExprInner::Sum(a, b) => {
                ExprInner::Sum(Box::new(a.map_external(&f)), Box::new(b.map_external(&f)))
            }
            ExprInner::ScalarProduct(c, a) => {
                ExprInner::ScalarProduct(*c, Box::new(a.map_external(&f)))
            }
        })
    }

    pub fn map_local<Sym2>(&self, f: &impl Fn(&Sym) -> Sym2) -> Expr<Ext, Sym2> {
        Expr(match &self.0 {
            ExprInner::Value(v) => ExprInner::Value(v.map_local(f)),
            ExprInner::Difference(a, b) => {
                ExprInner::Difference(Box::new(a.map_local(&f)), Box::new(b.map_local(&f)))
            }
            ExprInner::Sum(a, b) => {
                ExprInner::Sum(Box::new(a.map_local(&f)), Box::new(b.map_local(&f)))
            }
            ExprInner::ScalarProduct(c, a) => {
                ExprInner::ScalarProduct(*c, Box::new(a.map_local(&f)))
            }
        })
    }

    /// Attempts to partially resolve the expression, given the current
    /// resolver and types of the arguments.
    ///
    /// As this is done before the final build, all local addresses are assumed
    /// to have some base offset. For those values that can be computed, the
    /// expression is resolved to an immediate value.
    fn partial_local_resolve_inner<R: LocalResolver<Sym>>(
        &self,
        current_address: usize,
        resolver: &R,
    ) -> Option<(Self, IntermediateValue)> {
        match &self.0 {
            ExprInner::Value(leaf_value) => {
                let intermediate = leaf_value.partial_eval(resolver, current_address)?;
                Some((
                    Expr(ExprInner::Value(
                        if let Some(exact_value) = intermediate.exact_value() {
                            LeafValue::Immediate(exact_value)
                        } else {
                            leaf_value.clone()
                        },
                    )),
                    intermediate,
                ))
            }
            ExprInner::Difference(a, b) => {
                let (a, a_val) = a.partial_local_resolve_inner(current_address, resolver)?;
                let (b, b_val) = b.partial_local_resolve_inner(current_address, resolver)?;
                let result = a_val.sub(b_val);

                if let Some(exact_value) = result.exact_value() {
                    return Some((Expr::new_literal(exact_value), result));
                }

                Some((
                    Expr(ExprInner::Difference(Box::new(a), Box::new(b))),
                    result,
                ))
            }
            ExprInner::Sum(a, b) => {
                let (a, a_val) = a.partial_local_resolve_inner(current_address, resolver)?;
                let (b, b_val) = b.partial_local_resolve_inner(current_address, resolver)?;
                let result = a_val.add(b_val);

                if let Some(exact_value) = result.exact_value() {
                    return Some((Expr::new_literal(exact_value), result));
                }

                Some((Expr(ExprInner::Sum(Box::new(a), Box::new(b))), result))
            }
            ExprInner::ScalarProduct(coeff, expr) => {
                let (expr, val) = expr.partial_local_resolve_inner(current_address, resolver)?;
                let result = val.scalar_multiply((*coeff).safe_widen_to());

                if let Some(exact_value) = result.exact_value() {
                    return Some((Expr::new_literal(exact_value), result));
                }

                Some((
                    Expr(ExprInner::ScalarProduct(*coeff, Box::new(expr))),
                    result,
                ))
            }
        }
    }

    pub(super) fn partial_local_resolve<R: LocalResolver<Sym>>(
        &self,
        current_address: usize,
        resolver: &R,
    ) -> Option<Self> {
        let (result, _) = self.partial_local_resolve_inner(current_address, resolver)?;
        Some(result)
    }

    pub(super) fn exact_value(&self) -> Option<i64> {
        match &self.0 {
            ExprInner::Value(v) => v.exact_value(),
            _ => None,
        }
    }

    pub(super) fn full_resolve<R>(
        &self,
        current_address: usize,
        full_resolver: &R,
    ) -> anyhow::Result<i64>
    where
        Sym: std::fmt::Debug,
        R: super::FullResolver<Ext, Sym>,
    {
        match &self.0 {
            ExprInner::Value(leaf_value) => leaf_value.full_eval(full_resolver, current_address),
            ExprInner::Difference(a, b) => {
                let a_value = a.full_resolve(current_address, full_resolver)?;
                let b_value = b.full_resolve(current_address, full_resolver)?;
                a_value
                    .checked_sub(b_value)
                    .ok_or_else(|| anyhow::anyhow!("subtraction overflow in relocation expression"))
            }
            ExprInner::Sum(a, b) => {
                let a_value = a.full_resolve(current_address, full_resolver)?;
                let b_value = b.full_resolve(current_address, full_resolver)?;
                a_value
                    .checked_add(b_value)
                    .ok_or_else(|| anyhow::anyhow!("addition overflow in relocation expression"))
            }
            ExprInner::ScalarProduct(coeff, expr) => {
                let expr_value = expr.full_resolve(current_address, full_resolver)?;
                expr_value.checked_mul(*coeff).ok_or_else(|| {
                    anyhow::anyhow!("multiplication overflow in relocation expression")
                })
            }
        }
    }

    pub(super) fn filter_map_local<F, T>(self, body: &mut F) -> anyhow::Result<Expr<Ext, T>>
    where
        F: FnMut(Sym) -> Option<T>,
        T: Clone,
    {
        Ok(Expr(match self.0 {
            ExprInner::Value(v) => ExprInner::Value(v.filter_map_local(body)?),
            ExprInner::Difference(a, b) => {
                let a = a.filter_map_local(body)?;
                let b = b.filter_map_local(body)?;
                ExprInner::Difference(Box::new(a), Box::new(b))
            }
            ExprInner::Sum(a, b) => {
                let a = a.filter_map_local(body)?;
                let b = b.filter_map_local(body)?;
                ExprInner::Sum(Box::new(a), Box::new(b))
            }
            ExprInner::ScalarProduct(coeff, expr) => {
                let expr = expr.filter_map_local(body)?;
                ExprInner::ScalarProduct(coeff, Box::new(expr))
            }
        }))
    }
}
