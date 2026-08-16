use crate::{DispatchError, ParamError, SystemParam, param::validate_accesses};

/// Adapts a typed command function to a uniform resource-aware invocation.
///
/// `Params` is a tuple of [`SystemParam`] marker types. Implementations are
/// provided for functions with zero through eight injected arguments. Most
/// applications create the adapter through [`crate::command!`].
pub trait Handler<C, R, O, E, Params> {
    /// Validates resource access, extracts arguments, and invokes the function.
    fn run(&self, context: &mut C, resources: &mut R) -> Result<O, DispatchError<E>>;
}

impl<C, R, O, E, F> Handler<C, R, O, E, ()> for F
where
    F: Fn(&mut C) -> Result<O, E>,
{
    fn run(&self, context: &mut C, _resources: &mut R) -> Result<O, DispatchError<E>> {
        self(context).map_err(DispatchError::Command)
    }
}

macro_rules! impl_handler {
    ($($param:ident => $value:ident @ $index:expr),+ $(,)?) => {
        impl<C, R, O, E, F, $($param),+> Handler<C, R, O, E, ($($param,)+)> for F
        where
            $($param: SystemParam<R>,)+
            for<'context, 'resources> F:
                Fn(&'context mut C, $($param::Item<'resources>),+) -> Result<O, E>,
        {
            fn run<'resources>(
                &self,
                context: &mut C,
                resources: &'resources mut R,
            ) -> Result<O, DispatchError<E>> {
                let accesses = [$($param::ACCESS),+];
                validate_accesses(&accesses).map_err(DispatchError::Param)?;

                let resources = resources as *mut R;

                $(
                    // SAFETY: the pointer comes from an exclusive borrow that
                    // lasts for `'resources`, and all declared accesses were
                    // checked for conflicts before any parameter was fetched.
                    let $value: $param::Item<'resources> = unsafe {
                        $param::fetch::<'resources>(resources)
                    }
                    .map_err(|error| {
                        DispatchError::Param(ParamError::Fetch {
                            index: $index,
                            error,
                        })
                    })?;
                )+

                self(context, $($value),+).map_err(DispatchError::Command)
            }
        }
    };
}

impl_handler!(P0 => p0 @ 0);
impl_handler!(P0 => p0 @ 0, P1 => p1 @ 1);
impl_handler!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2);
impl_handler!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2, P3 => p3 @ 3);
impl_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
);
impl_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
);
impl_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
);
impl_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
    P7 => p7 @ 7,
);
