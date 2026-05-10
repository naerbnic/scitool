pub(crate) trait ResultExt {
    type Ok;
    type Err;

    fn merge_err<T2, E2>(self) -> Result<T2, E2>
    where
        Self::Ok: ResultExt,
        Result<T2, E2>: From<Self::Ok>,
        E2: From<Self::Err>;

    fn merge_err_by<F, T2, E2>(self, f: F) -> Result<T2, E2>
    where
        Self::Ok: ResultExt,
        Result<T2, E2>: From<Self::Ok>,
        F: FnOnce(Self::Err) -> E2;
}

impl<T, E> ResultExt for Result<T, E> {
    type Ok = T;
    type Err = E;

    fn merge_err<T2, E2>(self) -> Result<T2, E2>
    where
        <Self as ResultExt>::Ok: ResultExt,
        Result<T2, E2>: From<<Self as ResultExt>::Ok>,
        E2: From<<Self as ResultExt>::Err>,
    {
        match self {
            Ok(ok) => ok.into(),
            Err(err) => Err(err.into()),
        }
    }

    fn merge_err_by<F, T2, E2>(self, f: F) -> Result<T2, E2>
    where
        <Self as ResultExt>::Ok: ResultExt,
        Result<T2, E2>: From<T>,
        F: FnOnce(<Self as ResultExt>::Err) -> E2,
    {
        match self {
            Ok(ok) => ok.into(),
            Err(err) => Err(f(err)),
        }
    }
}
