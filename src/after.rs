use core::marker::PhantomData;

use crate::{StageError, SystemParam, param::ParamSet};

/// Adapts a typed function that runs after a successful command.
pub trait AfterHandler<C, R, O, E, Params> {
    /// Validates resource access, extracts arguments, and invokes the function.
    fn run(&self, context: &mut C, output: &mut O, resources: &mut R) -> Result<(), StageError<E>>;
}

impl<C, R, O, E, F> AfterHandler<C, R, O, E, ()> for F
where
    F: Fn(&mut C, &mut O) -> Result<(), E>,
{
    fn run(
        &self,
        context: &mut C,
        output: &mut O,
        _resources: &mut R,
    ) -> Result<(), StageError<E>> {
        self(context, output).map_err(StageError::User)
    }
}

macro_rules! impl_after_handler {
    ($($param:ident => $value:ident @ $index:expr),+ $(,)?) => {
        impl<C, R, O, E, F, $($param),+> AfterHandler<C, R, O, E, ($($param,)+)> for F
        where
            $($param: SystemParam<R>,)+
            F: Fn(&mut C, &mut O, $($param),+) -> Result<(), E>,
            for<'context, 'output, 'resources> F:
                Fn(
                    &'context mut C,
                    &'output mut O,
                    $($param::Item<'resources>),+
                ) -> Result<(), E>,
        {
            fn run(
                &self,
                context: &mut C,
                output: &mut O,
                resources: &mut R,
            ) -> Result<(), StageError<E>> {
                let ($($value,)+) =
                    <($($param,)+) as ParamSet<R>>::fetch(resources)
                        .map_err(StageError::Param)?;

                self(context, output, $($value),+).map_err(StageError::User)
            }
        }
    };
}

impl_after_handler!(P0 => p0 @ 0);
impl_after_handler!(P0 => p0 @ 0, P1 => p1 @ 1);
impl_after_handler!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2);
impl_after_handler!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2, P3 => p3 @ 3);
impl_after_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
);
impl_after_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
);
impl_after_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
);
impl_after_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
    P7 => p7 @ 7,
);

/// Runs optional logic after a command succeeds.
pub trait After<C, R, O, E> {
    /// Runs the after stage with mutable access to the command output.
    fn run_after(
        &self,
        context: &mut C,
        output: &mut O,
        resources: &mut R,
    ) -> Result<(), StageError<E>>;
}

/// An omitted after stage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoAfter;

impl<C, R, O, E> After<C, R, O, E> for NoAfter {
    fn run_after(
        &self,
        _context: &mut C,
        _output: &mut O,
        _resources: &mut R,
    ) -> Result<(), StageError<E>> {
        Ok(())
    }
}

/// Adapts one resource-injected function into an [`After`] stage.
pub struct AfterFn<F, Params> {
    function: F,
    marker: PhantomData<fn() -> Params>,
}

impl<F, Params> AfterFn<F, Params> {
    pub(crate) const fn new(function: F) -> Self {
        Self {
            function,
            marker: PhantomData,
        }
    }

    /// Returns the wrapped function.
    pub const fn function(&self) -> &F {
        &self.function
    }

    /// Returns a mutable reference to the wrapped function.
    pub fn function_mut(&mut self) -> &mut F {
        &mut self.function
    }

    /// Consumes the adapter and returns the wrapped function.
    pub fn into_inner(self) -> F {
        self.function
    }
}

impl<C, R, O, E, F, Params> After<C, R, O, E> for AfterFn<F, Params>
where
    F: AfterHandler<C, R, O, E, Params>,
{
    fn run_after(
        &self,
        context: &mut C,
        output: &mut O,
        resources: &mut R,
    ) -> Result<(), StageError<E>> {
        self.function.run(context, output, resources)
    }
}
