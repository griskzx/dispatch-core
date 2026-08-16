use core::{error::Error, fmt};

use crate::ResourceId;

/// A failure reported while extracting one resource parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FetchError {
    /// The requested capability is not currently available.
    Unavailable,

    /// The resource backend could not grant the requested borrow.
    BorrowConflict,

    /// An application-defined, allocation-free error code.
    Custom(u16),
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("resource is unavailable"),
            Self::BorrowConflict => formatter.write_str("resource borrow conflicts"),
            Self::Custom(code) => write!(formatter, "resource backend error {code}"),
        }
    }
}

impl Error for FetchError {}

/// A resource-parameter preparation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamError {
    /// Two parameters requested incompatible access to one resource.
    AccessConflict {
        /// Zero-based position of the earlier parameter.
        first: u8,

        /// Zero-based position of the later parameter.
        second: u8,

        /// Identity shared by the conflicting accesses.
        resource: ResourceId,
    },

    /// A parameter backend failed while fetching an argument.
    Fetch {
        /// Zero-based position of the parameter.
        index: u8,

        /// Error returned by the parameter backend.
        error: FetchError,
    },
}

impl fmt::Display for ParamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessConflict {
                first,
                second,
                resource,
            } => write!(
                formatter,
                "parameters {first} and {second} conflict on resource {}",
                resource.get()
            ),
            Self::Fetch { index, error } => {
                write!(formatter, "failed to fetch parameter {index}: {error}")
            }
        }
    }
}

impl Error for ParamError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AccessConflict { .. } => None,
            Self::Fetch { error, .. } => Some(error),
        }
    }
}

/// An error raised while selecting or executing a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError<E> {
    /// No command in the table matched the current context.
    NotFound,

    /// Resource arguments could not be prepared safely.
    Param(ParamError),

    /// The selected command returned an application error.
    Command(E),
}

impl<E> fmt::Display for DispatchError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("no matching command"),
            Self::Param(error) => write!(formatter, "command parameter error: {error}"),
            Self::Command(error) => write!(formatter, "command execution failed: {error}"),
        }
    }
}

impl<E> Error for DispatchError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotFound => None,
            Self::Param(error) => Some(error),
            Self::Command(error) => Some(error),
        }
    }
}
