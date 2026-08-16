use core::marker::PhantomData;

use crate::{Handler, StageError};

/// Runs optional logic after command selection and before command execution.
pub trait Before<C, R, E> {
    /// Runs the before stage.
    fn run_before(&self, context: &mut C, resources: &mut R) -> Result<(), StageError<E>>;
}

/// An omitted before stage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoBefore;

impl<C, R, E> Before<C, R, E> for NoBefore {
    fn run_before(&self, _context: &mut C, _resources: &mut R) -> Result<(), StageError<E>> {
        Ok(())
    }
}

/// Adapts one resource-injected function into a [`Before`] stage.
pub struct BeforeFn<F, Params> {
    function: F,
    marker: PhantomData<fn() -> Params>,
}

impl<F, Params> BeforeFn<F, Params> {
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

impl<C, R, E, F, Params> Before<C, R, E> for BeforeFn<F, Params>
where
    F: Handler<C, R, (), E, Params>,
{
    fn run_before(&self, context: &mut C, resources: &mut R) -> Result<(), StageError<E>> {
        self.function.run(context, resources)
    }
}
