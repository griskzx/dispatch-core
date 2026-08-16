#![allow(dead_code)]

use core::marker::PhantomData;

/// Resource injection error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamError {
    Failed,
}

/// Application context is owned by caller.
pub trait SystemParam<'a, R> {
    type Item;

    fn fetch(resources: &'a R) -> Result<Self::Item, ParamError>;
}

/// Command handler abstraction.
pub trait Handler<C, R, O, E> {
    fn run(&self, context: &mut C, resources: &R) -> Result<O, E>;
}

pub struct Handler0<F>(pub F);

impl<C, R, O, E, F> Handler<C, R, O, E> for Handler0<F>
where
    F: Fn(&mut C) -> Result<O, E>,
{
    fn run(&self, context: &mut C, _resources: &R) -> Result<O, E> {
        (self.0)(context)
    }
}

pub struct Handler1<F, P>(pub F, PhantomData<P>);

impl<C, R, O, E, F, P> Handler<C, R, O, E> for Handler1<F, P>
where
    for<'a> P: SystemParam<'a, R>,
    for<'a> F: Fn(&mut C, <P as SystemParam<'a, R>>::Item) -> Result<O, E>,
{
    fn run(&self, context: &mut C, resources: &R) -> Result<O, E> {
        let p = P::fetch(resources).map_err(|_| panic!())?;
        (self.0)(context, p)
    }
}

/// A static-table command entry.
pub struct CommandEntry<C, R, O, E> {
    pub key: u32,
    pub invoke: fn(&mut C, &R) -> Result<O, E>,
}

pub fn dispatch_table<C, R, O, E>(
    table: &[CommandEntry<C, R, O, E>],
    key: u32,
    context: &mut C,
    resources: &R,
) -> Result<O, E>
{
    for command in table {
        if command.key == key {
            return (command.invoke)(context, resources);
        }
    }

    Err(unsafe { core::mem::zeroed() })
}
