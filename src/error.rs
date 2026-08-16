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

    /// The resource backend returned a null or misaligned pointer.
    InvalidPointer,

    /// An application-defined, allocation-free error code.
    Custom(u16),
}

impl fmt::Display for FetchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("resource is unavailable"),
            Self::BorrowConflict => formatter.write_str("resource borrow conflicts"),
            Self::InvalidPointer => formatter.write_str("resource pointer is invalid"),
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

/// One stage in the dispatch execution pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStage {
    /// Logic run after selection and before the command.
    Before,

    /// The selected command function.
    Command,

    /// Logic run after a successful command.
    After,

    /// Final successful-response transformation.
    Response,
}

impl fmt::Display for DispatchStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Before => formatter.write_str("before"),
            Self::Command => formatter.write_str("command"),
            Self::After => formatter.write_str("after"),
            Self::Response => formatter.write_str("response"),
        }
    }
}

/// A failure raised while preparing or calling one typed function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageError<E> {
    /// Resource arguments could not be prepared safely.
    Param(ParamError),

    /// The application function returned an error.
    User(E),
}

impl<E> fmt::Display for StageError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Param(error) => write!(formatter, "parameter error: {error}"),
            Self::User(error) => write!(formatter, "function failed: {error}"),
        }
    }
}

impl<E> Error for StageError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Param(error) => Some(error),
            Self::User(error) => Some(error),
        }
    }
}

/// An error raised while selecting or executing a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError<E> {
    /// No command in the table matched the current context.
    NotFound,

    /// Resource arguments for one pipeline stage could not be prepared safely.
    Param {
        /// Stage whose parameters could not be prepared.
        stage: DispatchStage,

        /// Parameter validation or fetch failure.
        error: ParamError,
    },

    /// The configured before hook returned an application error.
    Before(E),

    /// The selected command returned an application error.
    Command(E),

    /// The configured after hook returned an application error.
    After(E),

    /// The configured response handler returned an application error.
    Response(E),
}

impl<E> DispatchError<E> {
    pub(crate) fn at(stage: DispatchStage, error: StageError<E>) -> Self {
        match error {
            StageError::Param(error) => Self::Param { stage, error },
            StageError::User(error) => match stage {
                DispatchStage::Before => Self::Before(error),
                DispatchStage::Command => Self::Command(error),
                DispatchStage::After => Self::After(error),
                DispatchStage::Response => Self::Response(error),
            },
        }
    }
}

impl<E> fmt::Display for DispatchError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("no matching command"),
            Self::Param { stage, error } => write!(formatter, "{stage} parameter error: {error}"),
            Self::Before(error) => write!(formatter, "before hook failed: {error}"),
            Self::Command(error) => write!(formatter, "command execution failed: {error}"),
            Self::After(error) => write!(formatter, "after hook failed: {error}"),
            Self::Response(error) => write!(formatter, "response handling failed: {error}"),
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
            Self::Param { error, .. } => Some(error),
            Self::Before(error)
            | Self::Command(error)
            | Self::After(error)
            | Self::Response(error) => Some(error),
        }
    }
}
