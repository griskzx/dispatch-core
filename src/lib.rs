#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

mod command;
mod error;
mod handler;
mod param;

pub use command::{
    CommandEntry, CommandHandler, CommandKey, Dispatcher, KeyMatcher, Matcher, dispatch,
};
pub use error::{DispatchError, FetchError, ParamError};
pub use handler::Handler;
pub use param::{AccessKind, ResourceAccess, ResourceId, SystemParam};

/// Builds a statically dispatchable command entry from a function.
///
/// A command without injected resources only needs its key and function:
///
/// ```text
/// command!(0x01, version)
/// ```
///
/// Resource parameters are listed after a semicolon in the same order as the
/// function parameters:
///
/// ```text
/// command!(0x02, send; IoMut)
/// command!(0x03, sign; CryptoRef, IoMut)
/// ```
///
/// The parameter marker types implement [`SystemParam`]. The generated entry
/// contains only the key and a monomorphized function pointer, so an array of
/// entries can be stored in read-only memory.
#[macro_export]
macro_rules! command {
    ($key:expr, $handler:path $(,)?) => {
        $crate::CommandEntry::new($key, |context, resources| {
            $crate::__private::run_handler::<(), _, _, _, _, _>(
                $handler, context, resources,
            )
        })
    };
    ($key:expr, $handler:path; $($param:ty),+ $(,)?) => {
        $crate::CommandEntry::new($key, |context, resources| {
            $crate::__private::run_handler::<($($param,)+), _, _, _, _, _>(
                $handler, context, resources,
            )
        })
    };
}

/// Declares a [`SystemParam`] marker for direct access to a resource field.
///
/// Shared and exclusive markers for the same field automatically receive the
/// same resource identity, allowing the handler to detect conflicting access
/// before creating references.
///
/// # Examples
///
/// ```
/// use dispatch_core::resource_param;
///
/// struct Resources {
///     counter: u32,
/// }
///
/// resource_param!(CounterRef for Resources => counter: u32, shared);
/// resource_param!(CounterMut for Resources => counter: u32, exclusive);
/// ```
///
/// The generated implementation is intended for ordinary, non-packed struct
/// fields. Implement [`SystemParam`] manually when extraction requires a
/// handle, guard, fallible lookup, or a different access model.
#[macro_export]
macro_rules! resource_param {
    ($(#[$meta:meta])* $vis:vis $marker:ident for $resources:ty => $field:ident : $item:ty, shared $(,)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default)]
        $vis struct $marker;

        // SAFETY: `ACCESS` and `fetch` are generated from the same field, and
        // shared access only creates a shared reference.
        unsafe impl $crate::SystemParam<$resources> for $marker {
            type Item<'resources> = &'resources $item;

            const ACCESS: $crate::ResourceAccess = $crate::ResourceAccess::shared(
                $crate::ResourceId::new(::core::mem::offset_of!($resources, $field)),
            );

            unsafe fn fetch<'resources>(
                resources: *mut $resources,
            ) -> ::core::result::Result<Self::Item<'resources>, $crate::FetchError> {
                // SAFETY: guaranteed by the `SystemParam` implementation
                // contract and the handler's access-conflict validation.
                ::core::result::Result::Ok(unsafe { &(*resources).$field })
            }
        }
    };
    ($(#[$meta:meta])* $vis:vis $marker:ident for $resources:ty => $field:ident : $item:ty, exclusive $(,)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default)]
        $vis struct $marker;

        // SAFETY: `ACCESS` and `fetch` are generated from the same field, and
        // the handler rejects every overlapping exclusive access first.
        unsafe impl $crate::SystemParam<$resources> for $marker {
            type Item<'resources> = &'resources mut $item;

            const ACCESS: $crate::ResourceAccess = $crate::ResourceAccess::exclusive(
                $crate::ResourceId::new(::core::mem::offset_of!($resources, $field)),
            );

            unsafe fn fetch<'resources>(
                resources: *mut $resources,
            ) -> ::core::result::Result<Self::Item<'resources>, $crate::FetchError> {
                // SAFETY: guaranteed by the `SystemParam` implementation
                // contract and the handler's access-conflict validation.
                ::core::result::Result::Ok(unsafe { &mut (*resources).$field })
            }
        }
    };
}

/// Implementation details used by exported macros.
///
/// This module is public only because macros expand in downstream crates. Its
/// contents are not part of the stable API.
#[doc(hidden)]
pub mod __private {
    use crate::{DispatchError, Handler};

    pub fn run_handler<Params, C, R, O, E, H>(
        handler: H,
        context: &mut C,
        resources: &mut R,
    ) -> Result<O, DispatchError<E>>
    where
        H: Handler<C, R, O, E, Params>,
    {
        handler.run(context, resources)
    }
}
