//! A relocation entry may be the result of more than one symbol reference.
//! We allow the relocation values to be simple expressions, which are
//! partially evaluated at the time of the relocation.

use crate::{
    numbers::bit_convert::{NumConvert, WidenTo},
    symbol::Symbol,
};

use super::LocalResolver;

/// A value for a symbol, before it has been fully resolved.
#[derive(Clone, Copy, Debug)]
struct IntermediateValue {
    base_address_coefficient: i64,
    // An absolute offset from the base address.
    offset: i64,
}

impl IntermediateValue {
    pub fn new_base_relative(offset: i64) -> Self {
        IntermediateValue {
            base_address_coefficient: 1,
            offset,
        }
    }

    pub fn new_exact(offset: i64) -> Self {
        IntermediateValue {
            base_address_coefficient: 0,
            offset,
        }
    }

    pub fn add(self, other: Self) -> Self {
        IntermediateValue {
            base_address_coefficient: self
                .base_address_coefficient
                .checked_add(other.base_address_coefficient)
                .expect("overflow in addition"),
            offset: self
                .offset
                .checked_add(other.offset)
                .expect("overflow in addition"),
        }
    }

    pub fn sub(self, other: Self) -> Self {
        IntermediateValue {
            base_address_coefficient: self
                .base_address_coefficient
                .checked_sub(other.base_address_coefficient)
                .expect("overflow in subtraction"),
            offset: self
                .offset
                .checked_sub(other.offset)
                .expect("overflow in subtraction"),
        }
    }

    pub fn scalar_multiply(self, other: i64) -> Self {
        IntermediateValue {
            base_address_coefficient: self
                .base_address_coefficient
                .checked_mul(other)
                .expect("overflow in scalar multiplication"),
            offset: self
                .offset
                .checked_mul(other)
                .expect("overflow in scalar multiplication"),
        }
    }

    pub fn exact_value(&self) -> Option<i64> {
        if self.base_address_coefficient == 0 {
            Some(self.offset)
        } else {
            None
        }
    }

    pub fn eval_with_base_address(&self, base: i64) -> i64 {
        base.checked_mul(self.base_address_coefficient)
            .and_then(|base_offset| base_offset.checked_add(self.offset))
            .expect("overflow in relocation evaluation")
    }

    fn with_added_offset(&self, offset: i64) -> IntermediateValue {
        IntermediateValue {
            base_address_coefficient: self.base_address_coefficient,
            offset: self
                .base_address_coefficient
                .checked_mul(offset)
                .and_then(|value_offset| self.offset.checked_add(value_offset))
                .expect("overflow in relocation evaluation"),
        }
    }
}

/// A primitive expression value that yields a single value.
#[derive(Clone, Debug)]
enum LeafValue {
    /// The current address of the relocation entry.
    CurrentAddress,

    /// An immediate value.
    ///
    /// This can be an exact value, or a value that is a linear combination of
    /// an offset and a multiple of the base address.
    Immediate(IntermediateValue),

    /// The location value of the symbol within this relocatable buffer.
    LocalSymbol(Symbol),

    /// A value that comes from an external source, and may not be an
    /// address.
    ExternalValue(Symbol),
}

impl LeafValue {
    fn partial_eval<R>(&self, resolver: &R, current_address: usize) -> Option<IntermediateValue>
    where
        R: LocalResolver,
    {
        match self {
            LeafValue::CurrentAddress => Some(IntermediateValue::new_base_relative(
                current_address.convert_num_to().unwrap(),
            )),
            LeafValue::Immediate(value) => Some(*value),
            LeafValue::LocalSymbol(sym) => Some(IntermediateValue::new_base_relative(
                resolver.resolve_local_symbol(sym)?,
            )),
            LeafValue::ExternalValue(_) => None,
        }
    }

    pub fn full_eval<R>(&self, resolver: &R, current_address: usize) -> anyhow::Result<i64>
    where
        R: super::FullResolver,
    {
        match self {
            LeafValue::CurrentAddress => current_address.convert_num_to(),
            LeafValue::Immediate(value) => Ok(value.eval_with_base_address(0)),
            LeafValue::LocalSymbol(sym) => resolver
                .resolve_local_symbol(sym)
                .ok_or_else(|| anyhow::anyhow!("failed to resolve local symbol {:?}", sym)),
            LeafValue::ExternalValue(value) => resolver.resolve(value),
        }
    }

    pub fn exact_value(&self) -> Option<i64> {
        match self {
            LeafValue::Immediate(value) => value.exact_value(),
            _ => None,
        }
    }
    pub fn filter_map_local<F>(self, mut body: F) -> anyhow::Result<LeafValue>
    where
        F: FnMut(Symbol) -> Option<Symbol>,
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

impl LeafValue {
    pub fn map_external(&self, f: &impl Fn(&Symbol) -> Symbol) -> LeafValue {
        match self {
            LeafValue::CurrentAddress => LeafValue::CurrentAddress,
            LeafValue::Immediate(value) => LeafValue::Immediate(*value),
            LeafValue::LocalSymbol(sym) => LeafValue::LocalSymbol(sym.clone()),
            LeafValue::ExternalValue(value) => LeafValue::ExternalValue(f(value)),
        }
    }

    pub fn map_local(&self, f: &impl Fn(&Symbol) -> Symbol) -> LeafValue {
        match self {
            LeafValue::CurrentAddress => LeafValue::CurrentAddress,
            LeafValue::Immediate(value) => LeafValue::Immediate(*value),
            LeafValue::LocalSymbol(sym) => LeafValue::LocalSymbol(f(sym)),
            LeafValue::ExternalValue(value) => LeafValue::ExternalValue(value.clone()),
        }
    }

    fn with_added_offset(&self, offset: i64) -> Self {
        match self {
            LeafValue::CurrentAddress => LeafValue::CurrentAddress,
            LeafValue::Immediate(intermediate_value) => {
                LeafValue::Immediate(intermediate_value.with_added_offset(offset))
            }
            LeafValue::LocalSymbol(sym) => LeafValue::LocalSymbol(sym.clone()),
            LeafValue::ExternalValue(ext) => LeafValue::ExternalValue(ext.clone()),
        }
    }
}

#[derive(Clone, Debug)]
enum ExprInner {
    Value(LeafValue),
    Difference(Box<Expr>, Box<Expr>),
    Sum(Box<Expr>, Box<Expr>),
    ScalarProduct(i64, Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Expr(ExprInner);

impl Expr {
    pub fn new_local(symbol: Symbol) -> Self {
        Expr(ExprInner::Value(LeafValue::LocalSymbol(symbol)))
    }

    pub fn new_external(value: Symbol) -> Self {
        Expr(ExprInner::Value(LeafValue::ExternalValue(value)))
    }

    pub fn new_literal(value: i64) -> Self {
        Expr(ExprInner::Value(LeafValue::Immediate(
            IntermediateValue::new_exact(value),
        )))
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

    pub fn local_symbols(&self) -> impl Iterator<Item = &Symbol> {
        match &self.0 {
            ExprInner::Value(LeafValue::LocalSymbol(sym)) => {
                Box::new(std::iter::once(sym)) as Box<dyn Iterator<Item = &Symbol>>
            }
            ExprInner::Value(_) => {
                Box::new(std::iter::empty()) as Box<dyn Iterator<Item = &Symbol>>
            }
            ExprInner::Difference(a, b) | ExprInner::Sum(a, b) => {
                Box::new(a.local_symbols().chain(b.local_symbols()))
                    as Box<dyn Iterator<Item = &Symbol>>
            }
            ExprInner::ScalarProduct(_, a) => a.local_symbols(),
        }
    }

    pub fn map_external(&self, f: &impl Fn(&Symbol) -> Symbol) -> Expr {
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

    pub fn map_local(&self, f: &impl Fn(&Symbol) -> Symbol) -> Expr {
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
    fn partial_local_resolve_inner<R: LocalResolver>(
        &self,
        current_address: usize,
        resolver: &R,
    ) -> Option<(Self, IntermediateValue)> {
        match &self.0 {
            ExprInner::Value(leaf_value) => {
                let intermediate = leaf_value.partial_eval(resolver, current_address)?;
                Some((
                    Expr(ExprInner::Value(LeafValue::Immediate(intermediate))),
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

    pub(super) fn partial_local_resolve<R: LocalResolver>(
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
        R: super::FullResolver,
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

    pub(super) fn filter_map_local<F>(self, body: &mut F) -> anyhow::Result<Expr>
    where
        F: FnMut(Symbol) -> Option<Symbol>,
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

    pub(super) fn with_added_offset(&self, offset: i64) -> Self {
        Expr(match &self.0 {
            ExprInner::Value(leaf_value) => ExprInner::Value(leaf_value.with_added_offset(offset)),
            ExprInner::Difference(expr, expr1) => ExprInner::Difference(
                Box::new(expr.with_added_offset(offset)),
                Box::new(expr1.with_added_offset(offset)),
            ),
            ExprInner::Sum(expr, expr1) => ExprInner::Sum(
                Box::new(expr.with_added_offset(offset)),
                Box::new(expr1.with_added_offset(offset)),
            ),
            ExprInner::ScalarProduct(coeff, expr) => {
                ExprInner::ScalarProduct(*coeff, Box::new(expr.with_added_offset(offset)))
            }
        })
    }
}
