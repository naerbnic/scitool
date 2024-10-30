//! A relocation entry may be the result of more than one symbol reference.
//! We allow the relocation values to be simple expressions, which are
//! partially evaluated at the time of the relocation.

pub trait Resolver<Ext, Sym> {
    fn resolve_local_symbol(&self, symbol: &Sym) -> Option<usize>;
    fn resolve_external_value(&self, value: &Ext) -> Option<usize>;
}

/// The coefficient for a symbol address for the base of the current block.
///
/// All addresses in a non-finalized buffer are relative to the start of the
/// buffer. This means that the base address of the buffer is the current
/// address of the buffer. This value tracks the coefficient of the base
/// address through various math operations, or states if the actual
/// coefficient is unknown.
#[derive(Clone, Copy, Debug)]
enum BaseCoefficient {
    Known(isize),
    Unknown,
}

/// A value for a symbol, before it has been fully resolved.
#[derive(Clone, Copy, Debug)]
struct IntermediateValue(Option<(isize, isize)>);

impl IntermediateValue {
    pub fn new_base_relative(offset: isize) -> Self {
        IntermediateValue(Some((offset, 1)))
    }

    pub fn new_exact(offset: isize) -> Self {
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

    pub fn scalar_multiply(self, other: isize) -> Self {
        match self.0 {
            Some((a, b)) => IntermediateValue(Some((a * other, b * other))),
            None => IntermediateValue(None),
        }
    }

    pub fn offset(&self) -> Option<isize> {
        self.0.map(|(a, _)| a)
    }

    pub fn base_coefficient(&self) -> Option<isize> {
        self.0.map(|(_, b)| b)
    }

    pub fn is_known(&self) -> bool {
        self.0.is_some()
    }

    pub fn exact_value(&self) -> Option<isize> {
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
pub enum LeafValue<Ext, Sym> {
    /// The current address of the relocation entry.
    CurrentAddress,

    /// An immediate value.
    Immediate(usize),

    /// The location value of the symbol within this relocatable buffer.
    LocalSymbol(Sym),

    /// A value that comes from an external source, and may not be an
    /// address.
    ExternalValue(Ext),
}

impl<Ext, Sym> LeafValue<Ext, Sym> {
    pub fn resolve<R: Resolver<Ext, Sym>>(
        &self,
        current_address: usize,
        resolver: &R,
    ) -> Option<usize> {
        match self {
            LeafValue::CurrentAddress => Some(current_address),
            LeafValue::Immediate(value) => Some(*value),
            LeafValue::LocalSymbol(symbol) => resolver.resolve_local_symbol(symbol),
            LeafValue::ExternalValue(value) => resolver.resolve_external_value(value),
        }
    }

    fn partial_eval<R>(&self, resolver: &R, current_address: usize) -> Option<IntermediateValue>
    where
        R: Resolver<Ext, Sym>,
    {
        let result = match self {
            LeafValue::CurrentAddress => {
                IntermediateValue::new_base_relative(current_address.try_into().unwrap())
            }
            LeafValue::Immediate(value) => {
                IntermediateValue::new_exact((*value).try_into().unwrap())
            }
            LeafValue::LocalSymbol(sym) => IntermediateValue::new_base_relative(
                resolver.resolve_local_symbol(sym)?.try_into().unwrap(),
            ),
            LeafValue::ExternalValue(_) => IntermediateValue::new_unknown(),
        };
        Some(result)
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
pub enum Expr<Ext, Sym> {
    Value(LeafValue<Ext, Sym>),
    Difference(Box<Expr<Ext, Sym>>, Box<Expr<Ext, Sym>>),
    Sum(Box<Expr<Ext, Sym>>, Box<Expr<Ext, Sym>>),
    ScalarProduct(usize, Box<Expr<Ext, Sym>>),
}

impl<Ext, Sym> Expr<Ext, Sym>
where
    Ext: Clone,
    Sym: Clone,
{
    pub fn local_symbols(&self) -> impl Iterator<Item = &Sym> {
        match self {
            Expr::Value(LeafValue::LocalSymbol(sym)) => {
                Box::new(std::iter::once(sym)) as Box<dyn Iterator<Item = &Sym>>
            }
            Expr::Value(_) => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = &Sym>>,
            Expr::Difference(a, b) | Expr::Sum(a, b) => {
                Box::new(a.local_symbols().chain(b.local_symbols()))
                    as Box<dyn Iterator<Item = &Sym>>
            }
            Expr::ScalarProduct(_, a) => a.local_symbols(),
        }
    }

    pub fn map_external<Ext2>(&self, f: &impl Fn(&Ext) -> Ext2) -> Expr<Ext2, Sym> {
        match self {
            Expr::Value(v) => Expr::Value(v.map_external(f)),
            Expr::Difference(a, b) => {
                Expr::Difference(Box::new(a.map_external(&f)), Box::new(b.map_external(&f)))
            }
            Expr::Sum(a, b) => {
                Expr::Sum(Box::new(a.map_external(&f)), Box::new(b.map_external(&f)))
            }
            Expr::ScalarProduct(c, a) => Expr::ScalarProduct(*c, Box::new(a.map_external(&f))),
        }
    }

    pub fn map_local<Sym2>(&self, f: &impl Fn(&Sym) -> Sym2) -> Expr<Ext, Sym2> {
        match self {
            Expr::Value(v) => Expr::Value(v.map_local(f)),
            Expr::Difference(a, b) => {
                Expr::Difference(Box::new(a.map_local(&f)), Box::new(b.map_local(&f)))
            }
            Expr::Sum(a, b) => Expr::Sum(Box::new(a.map_local(&f)), Box::new(b.map_local(&f))),
            Expr::ScalarProduct(c, a) => Expr::ScalarProduct(*c, Box::new(a.map_local(&f))),
        }
    }

    fn partial_eval<R>(&self, resolver: &R) -> Option<IntermediateValue>
    where
        R: Resolver<Ext, Sym>,
    {
        match self {
            Expr::Value(leaf_value) => leaf_value.partial_eval(resolver, 0),
            Expr::Difference(a, b) => {
                let a = a.partial_eval(resolver)?;
                let b = b.partial_eval(resolver)?;
                Some(a.sub(b))
            }
            Expr::Sum(a, b) => {
                let a = a.partial_eval(resolver)?;
                let b = b.partial_eval(resolver)?;
                Some(a.add(b))
            }
            Expr::ScalarProduct(c, a) => {
                let a = a.partial_eval(resolver)?;
                Some(a.scalar_multiply(*c as isize))
            }
        }
    }

    /// Attempts to partially resolve the expression, given the current
    /// resolver and types of the arguments.
    ///
    /// As this is done before the final build, all local addresses are assumed
    /// to have some base offset. For those values that can be computed, the
    /// expression is resolved to an immediate value.
    fn partial_resolve_inner<R: Resolver<Ext, Sym>>(
        &self,
        current_address: usize,
        resolver: &R,
    ) -> Option<(Self, IntermediateValue)> {
        match self {
            Expr::Value(leaf_value) => {
                let intermediate = leaf_value.partial_eval(resolver, current_address)?;
                Some((
                    Expr::Value(if let Some(exact_value) = intermediate.exact_value() {
                        LeafValue::Immediate(exact_value.try_into().unwrap())
                    } else {
                        leaf_value.clone()
                    }),
                    intermediate,
                ))
            }
            Expr::Difference(a, b) => {
                let (a, a_val) = a.partial_resolve_inner(current_address, resolver)?;
                let (b, b_val) = b.partial_resolve_inner(current_address, resolver)?;
                let result = a_val.sub(b_val);

                if let Some(exact_value) = result.exact_value() {
                    return Some((
                        Expr::Value(LeafValue::Immediate(exact_value.try_into().unwrap())),
                        result,
                    ));
                }

                Some((Expr::Difference(Box::new(a), Box::new(b)), result))
            }
            Expr::Sum(a, b) => {
                let (a, a_val) = a.partial_resolve_inner(current_address, resolver)?;
                let (b, b_val) = b.partial_resolve_inner(current_address, resolver)?;
                let result = a_val.add(b_val);

                if let Some(exact_value) = result.exact_value() {
                    return Some((
                        Expr::Value(LeafValue::Immediate(exact_value.try_into().unwrap())),
                        result,
                    ));
                }

                Some((Expr::Sum(Box::new(a), Box::new(b)), result))
            }
            Expr::ScalarProduct(coeff, expr) => {
                let (expr, val) = expr.partial_resolve_inner(current_address, resolver)?;
                let result = val.scalar_multiply(isize::try_from(*coeff).unwrap());

                if let Some(exact_value) = result.exact_value() {
                    return Some((
                        Expr::Value(LeafValue::Immediate(exact_value.try_into().unwrap())),
                        result,
                    ));
                }

                Some((Expr::ScalarProduct(*coeff, Box::new(expr)), result))
            }
        }
    }

    pub fn partial_resolve<R: Resolver<Ext, Sym>>(
        &self,
        current_address: usize,
        resolver: &R,
    ) -> Option<Self> {
        let (result, _) = self.partial_resolve_inner(current_address, resolver)?;
        Some(result)
    }
}
