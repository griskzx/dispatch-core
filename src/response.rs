use core::marker::PhantomData;

use crate::{StageError, SystemParam, param::ParamSet};

/// Adapts a typed function that transforms a successful command response.
pub trait ResponseHandler<C, R, O, T, E, Params> {
    /// Validates resource access, extracts arguments, and transforms `output`.
    fn run(&self, context: &mut C, output: O, resources: &mut R) -> Result<T, StageError<E>>;
}

impl<C, R, O, T, E, F> ResponseHandler<C, R, O, T, E, ()> for F
where
    F: Fn(&mut C, O) -> Result<T, E>,
{
    fn run(&self, context: &mut C, output: O, _resources: &mut R) -> Result<T, StageError<E>> {
        self(context, output).map_err(StageError::User)
    }
}

macro_rules! impl_response_handler {
    ($($param:ident => $value:ident @ $index:expr),+ $(,)?) => {
        impl<C, R, O, T, E, F, $($param),+>
            ResponseHandler<C, R, O, T, E, ($($param,)+)> for F
        where
            $($param: SystemParam<R>,)+
            F: Fn(&mut C, O, $($param),+) -> Result<T, E>,
            for<'context, 'resources> F:
                Fn(
                    &'context mut C,
                    O,
                    $($param::Item<'resources>),+
                ) -> Result<T, E>,
        {
            fn run(
                &self,
                context: &mut C,
                output: O,
                resources: &mut R,
            ) -> Result<T, StageError<E>> {
                let ($($value,)+) =
                    <($($param,)+) as ParamSet<R>>::fetch(resources)
                        .map_err(StageError::Param)?;

                self(context, output, $($value),+).map_err(StageError::User)
            }
        }
    };
}

impl_response_handler!(P0 => p0 @ 0);
impl_response_handler!(P0 => p0 @ 0, P1 => p1 @ 1);
impl_response_handler!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2);
impl_response_handler!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2, P3 => p3 @ 3);
impl_response_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
);
impl_response_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
);
impl_response_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
);
impl_response_handler!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
    P7 => p7 @ 7,
);

/// Finalizes a successful command output before it is returned to the caller.
pub trait ResponseStage<C, R, O, E> {
    /// Value returned by [`crate::Dispatcher::dispatch`].
    type Output;

    /// Transforms the successful command output.
    fn run_response(
        &self,
        context: &mut C,
        output: O,
        resources: &mut R,
    ) -> Result<Self::Output, StageError<E>>;
}

/// The default response stage, which returns command output unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdentityResponse;

impl<C, R, O, E> ResponseStage<C, R, O, E> for IdentityResponse {
    type Output = O;

    fn run_response(
        &self,
        _context: &mut C,
        output: O,
        _resources: &mut R,
    ) -> Result<Self::Output, StageError<E>> {
        Ok(output)
    }
}

/// Adapts one resource-injected function into a [`ResponseStage`].
pub struct ResponseFn<F, Params, T> {
    function: F,
    marker: PhantomData<fn() -> (Params, T)>,
}

impl<F, Params, T> ResponseFn<F, Params, T> {
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

impl<C, R, O, T, E, F, Params> ResponseStage<C, R, O, E> for ResponseFn<F, Params, T>
where
    F: ResponseHandler<C, R, O, T, E, Params>,
{
    type Output = T;

    fn run_response(
        &self,
        context: &mut C,
        output: O,
        resources: &mut R,
    ) -> Result<Self::Output, StageError<E>> {
        self.function.run(context, output, resources)
    }
}
