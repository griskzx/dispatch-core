use crate::ParamError;

/// Stable identity of one logical resource within a resource container.
///
/// Equal identifiers mean that accesses may touch the same memory. Different
/// identifiers must mean that the returned values do not overlap. The
/// [`crate::resource_param!`] macro derives this value from a field offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(usize);

impl ResourceId {
    /// Creates an identity from an application-defined value.
    ///
    /// Manual [`SystemParam`] implementations must use the same value for all
    /// markers that may access overlapping memory.
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the underlying allocation-free identity value.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// The kind of access a system parameter requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessKind {
    /// The parameter does not borrow a resource.
    None,

    /// Any number of parameters may share this resource.
    Shared,

    /// Exactly one parameter may access this resource.
    Exclusive,
}

/// Static access metadata for one system parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceAccess {
    resource: ResourceId,
    kind: AccessKind,
}

impl ResourceAccess {
    /// Declares a parameter that does not borrow from the resource container.
    pub const fn none() -> Self {
        Self {
            resource: ResourceId::new(0),
            kind: AccessKind::None,
        }
    }

    /// Declares shared access to `resource`.
    pub const fn shared(resource: ResourceId) -> Self {
        Self {
            resource,
            kind: AccessKind::Shared,
        }
    }

    /// Declares exclusive access to `resource`.
    pub const fn exclusive(resource: ResourceId) -> Self {
        Self {
            resource,
            kind: AccessKind::Exclusive,
        }
    }

    /// Returns the logical resource identity.
    pub const fn resource(self) -> ResourceId {
        self.resource
    }

    /// Returns the requested access kind.
    pub const fn kind(self) -> AccessKind {
        self.kind
    }

    const fn conflicts(self, other: Self) -> bool {
        if matches!(self.kind, AccessKind::None) || matches!(other.kind, AccessKind::None) {
            return false;
        }

        self.resource.0 == other.resource.0
            && (matches!(self.kind, AccessKind::Exclusive)
                || matches!(other.kind, AccessKind::Exclusive))
    }
}

/// Extracts one typed argument from a resource container.
///
/// Marker types describe both the value returned to the command and the
/// memory the value may access. Handlers validate all [`ResourceAccess`]
/// declarations before calling [`SystemParam::fetch`].
///
/// Prefer [`crate::resource_param!`] for direct struct fields. A manual
/// implementation is useful for fallible resource maps, handles, guards, or
/// backends with application-specific lookup behavior.
///
/// # Safety
///
/// An implementation must uphold all of these requirements:
///
/// - every value returned by `fetch` is valid only for the lifetime represented
///   by `Item<'resources>` and does not outlive the pointed-to container;
/// - returned values access only the logical resource declared by `ACCESS`;
/// - all parameter implementations that may touch overlapping memory use the
///   same [`ResourceId`];
/// - `Shared` access never permits mutation except through a sound interior-
///   mutability abstraction, while `Exclusive` access may produce unique
///   mutable references;
/// - `fetch` does not move or invalidate the resource container.
pub unsafe trait SystemParam<R> {
    /// Argument passed to the command for one resource-container lifetime.
    type Item<'resources>;

    /// Memory access performed by this parameter.
    const ACCESS: ResourceAccess;

    /// Extracts the command argument from `resources`.
    ///
    /// # Safety
    ///
    /// `resources` must be non-null, properly aligned, and point to a valid `R`
    /// that remains exclusively borrowed for `'resources`. The caller must
    /// validate this parameter's access against every other live parameter
    /// before calling this function.
    unsafe fn fetch<'resources>(
        resources: *mut R,
    ) -> Result<Self::Item<'resources>, crate::FetchError>;
}

pub(crate) fn validate_accesses(accesses: &[ResourceAccess]) -> Result<(), ParamError> {
    let mut second = 1;

    while second < accesses.len() {
        let mut first = 0;

        while first < second {
            if accesses[first].conflicts(accesses[second]) {
                return Err(ParamError::AccessConflict {
                    first: first as u8,
                    second: second as u8,
                    resource: accesses[first].resource,
                });
            }

            first += 1;
        }

        second += 1;
    }

    Ok(())
}
