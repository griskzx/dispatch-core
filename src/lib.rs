#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

mod after;
mod before;
mod command;
mod error;
mod handler;
mod param;
mod response;

pub use after::{After, AfterFn, AfterHandler, NoAfter};
pub use before::{Before, BeforeFn, NoBefore};
pub use command::{
    CommandEntry, CommandHandler, CommandKey, Dispatcher, KeyMatcher, Matcher, ResponseDispatcher,
    dispatch,
};
pub use error::{DispatchError, DispatchStage, FetchError, ParamError, StageError};
pub use handler::Handler;
pub use param::{
    AccessKind, DefaultResourceTag, Res, ResMut, ResourceAccess, ResourceId, ResourceProvider,
    SystemParam,
};
pub use response::{IdentityResponse, ResponseFn, ResponseHandler, ResponseStage};

/// Builds a statically dispatchable command entry from a function.
///
/// The entry declares only the routing relationship. Resource requirements are
/// inferred from [`Res`] and [`ResMut`] parameters in the function signature.
///
/// ```text
/// command!(0x01 => version)
/// ```
///
/// The generated entry contains only the key and a monomorphized function
/// pointer, so an array of entries can be stored in read-only memory.
#[macro_export]
macro_rules! command {
    ($key:expr => $handler:path $(,)?) => {
        $crate::CommandEntry::new($key, |context, resources| {
            $crate::__private::run_handler::<_, _, _, _, _, _>($handler, context, resources)
        })
    };
}

/// Declares an application resource container and its typed field providers.
///
/// Each field type is available through [`Res<T>`](Res) and
/// [`ResMut<T>`](ResMut). A tag after `=>` distinguishes fields that have the
/// same type.
///
/// # Examples
///
/// ```
/// use dispatch_core::resources;
///
/// struct Primary;
/// struct Backup;
///
/// resources! {
///     struct Resources {
///         counter: u32,
///         primary_port: u16 => Primary,
///         backup_port: u16 => Backup,
///     }
/// }
/// ```
///
/// A type may occur only once without a tag. Implement [`ResourceProvider`]
/// manually when the container already exists or lookup requires a custom
/// backend.
///
/// ```compile_fail
/// use dispatch_core::resources;
///
/// resources! {
///     struct Ambiguous {
///         first: u32,
///         second: u32,
///     }
/// }
/// ```
#[macro_export]
macro_rules! resources {
    (
        $(#[$container_meta:meta])*
        $container_vis:vis struct $container:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis $field:ident : $field_type:ty $(=> $tag:ty)?
            ),* $(,)?
        }
    ) => {
        $(#[$container_meta])*
        $container_vis struct $container {
            $(
                $(#[$field_meta])*
                $field_vis $field: $field_type,
            )*
        }

        $(
            $crate::resources!(@provider $container, $field, $field_type $(, $tag)?);
        )*
    };
    (@provider $container:ident, $field:ident, $field_type:ty) => {
        // SAFETY: the identity and pointer projection are generated from the
        // same ordinary struct field.
        unsafe impl $crate::ResourceProvider<$field_type> for $container {
            const ID: $crate::ResourceId = $crate::ResourceId::new(
                ::core::mem::offset_of!($container, $field),
            );

            unsafe fn get(resources: *mut Self) -> *mut $field_type {
                // SAFETY: guaranteed by the `ResourceProvider::get` caller.
                unsafe { ::core::ptr::addr_of_mut!((*resources).$field) }
            }
        }
    };
    (@provider $container:ident, $field:ident, $field_type:ty, $tag:ty) => {
        // SAFETY: the identity and pointer projection are generated from the
        // same ordinary struct field.
        unsafe impl $crate::ResourceProvider<$field_type, $tag> for $container {
            const ID: $crate::ResourceId = $crate::ResourceId::new(
                ::core::mem::offset_of!($container, $field),
            );

            unsafe fn get(resources: *mut Self) -> *mut $field_type {
                // SAFETY: guaranteed by the `ResourceProvider::get` caller.
                unsafe { ::core::ptr::addr_of_mut!((*resources).$field) }
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
    use crate::{Handler, StageError};

    pub fn run_handler<Params, C, R, O, E, H>(
        handler: H,
        context: &mut C,
        resources: &mut R,
    ) -> Result<O, StageError<E>>
    where
        H: Handler<C, R, O, E, Params>,
    {
        handler.run(context, resources)
    }
}
