use core::{
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::ParamError;

/// Default identity tag used when a resource type occurs only once.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DefaultResourceTag;

/// Stable identity of one logical resource within a resource container.
///
/// Equal identifiers mean that accesses may touch the same memory. Different
/// identifiers must mean that returned values do not overlap. The
/// [`crate::resources!`] macro derives identities from field offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(usize);

impl ResourceId {
    /// Creates an identity from an application-defined value.
    ///
    /// Manual resource implementations must use the same value for every
    /// provider that may access overlapping memory.
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the allocation-free identity value.
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

/// Provides one typed resource from an application-owned container.
///
/// `Tag` distinguishes multiple fields with the same `T`. Applications should
/// normally use [`crate::resources!`], which generates these implementations
/// together with the resource container.
///
/// # Safety
///
/// Every implementation that may return overlapping memory must use the same
/// `ID`. A non-null, aligned pointer returned by `get` must point to a live `T`
/// inside `Self`, remain valid while the container is exclusively borrowed,
/// and must not be used to move the container field. Null or misaligned
/// pointers are reported as [`crate::FetchError::InvalidPointer`].
pub unsafe trait ResourceProvider<T: ?Sized + 'static, Tag: 'static = DefaultResourceTag> {
    /// Identity used to detect overlapping shared and exclusive access.
    const ID: ResourceId;

    /// Returns a pointer to the provided resource.
    ///
    /// # Safety
    ///
    /// `resources` must point to a live, properly aligned `Self` that is
    /// exclusively borrowed for the entire lifetime of any value derived from
    /// the returned pointer.
    unsafe fn get(resources: *mut Self) -> *mut T;
}

/// Shared view of one resource.
///
/// The resource type and optional tag identify the required capability in a
/// command signature. `Res` is a transparent borrowing wrapper and allocates
/// no memory.
#[repr(transparent)]
pub struct Res<'resources, T: ?Sized + 'static, Tag: 'static = DefaultResourceTag> {
    value: &'resources T,
    marker: PhantomData<fn() -> Tag>,
}

impl<T, Tag> Clone for Res<'_, T, Tag>
where
    T: ?Sized + 'static,
    Tag: 'static,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, Tag> Copy for Res<'_, T, Tag>
where
    T: ?Sized + 'static,
    Tag: 'static,
{
}

impl<'resources, T, Tag> Res<'resources, T, Tag>
where
    T: ?Sized + 'static,
    Tag: 'static,
{
    /// Returns the wrapped shared reference.
    pub const fn into_inner(self) -> &'resources T {
        self.value
    }
}

impl<T, Tag> Deref for Res<'_, T, Tag>
where
    T: ?Sized + 'static,
    Tag: 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T, Tag> fmt::Debug for Res<'_, T, Tag>
where
    T: fmt::Debug + ?Sized + 'static,
    Tag: 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}

/// Exclusive view of one resource.
///
/// The resource type and optional tag identify the required capability in a
/// command signature. `ResMut` is a transparent borrowing wrapper and
/// allocates no memory.
#[repr(transparent)]
pub struct ResMut<'resources, T: ?Sized + 'static, Tag: 'static = DefaultResourceTag> {
    value: &'resources mut T,
    marker: PhantomData<fn() -> Tag>,
}

impl<'resources, T, Tag> ResMut<'resources, T, Tag>
where
    T: ?Sized + 'static,
    Tag: 'static,
{
    /// Returns the wrapped exclusive reference.
    pub fn into_inner(self) -> &'resources mut T {
        self.value
    }
}

impl<T, Tag> Deref for ResMut<'_, T, Tag>
where
    T: ?Sized + 'static,
    Tag: 'static,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T, Tag> DerefMut for ResMut<'_, T, Tag>
where
    T: ?Sized + 'static,
    Tag: 'static,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<T, Tag> fmt::Debug for ResMut<'_, T, Tag>
where
    T: fmt::Debug + ?Sized + 'static,
    Tag: 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(formatter)
    }
}

/// Extracts one typed argument from a resource container.
///
/// Most commands use [`Res`] and [`ResMut`], which implement this trait through
/// [`ResourceProvider`]. Manual implementations support fallible maps, guards,
/// handles, and application-specific parameter types.
///
/// # Safety
///
/// An implementation must uphold all of these requirements:
///
/// - every returned value is valid only for `Item<'resources>` and does not
///   outlive the pointed-to container;
/// - returned values access only the logical resource declared by `ACCESS`;
/// - implementations that may touch overlapping memory use the same
///   [`ResourceId`];
/// - shared access never permits unsound mutation, while exclusive access may
///   produce unique mutable references;
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
    /// exclusively borrowed for `'resources`. The caller must validate this
    /// access against every other live parameter before calling `fetch`.
    unsafe fn fetch<'resources>(
        resources: *mut R,
    ) -> Result<Self::Item<'resources>, crate::FetchError>;
}

// SAFETY: `ResourceProvider` supplies a valid pointer and identity. Handler
// validation prevents an exclusive access from overlapping this shared view.
unsafe impl<'param, R, T, Tag> SystemParam<R> for Res<'param, T, Tag>
where
    R: ResourceProvider<T, Tag>,
    T: 'static,
    Tag: 'static,
{
    type Item<'resources> = Res<'resources, T, Tag>;

    const ACCESS: ResourceAccess = ResourceAccess::shared(R::ID);

    unsafe fn fetch<'resources>(
        resources: *mut R,
    ) -> Result<Self::Item<'resources>, crate::FetchError> {
        // SAFETY: delegated to the caller and `ResourceProvider` contract.
        let value = unsafe { R::get(resources) };

        if value.is_null() || !value.is_aligned() {
            return Err(crate::FetchError::InvalidPointer);
        }

        // SAFETY: the provider guarantees a live, aligned pointer, and access
        // validation has ruled out every overlapping mutable reference.
        Ok(Res {
            value: unsafe { &*value },
            marker: PhantomData,
        })
    }
}

// SAFETY: `ResourceProvider` supplies a valid pointer and identity. Handler
// validation guarantees that this is the only access to the same resource.
unsafe impl<'param, R, T, Tag> SystemParam<R> for ResMut<'param, T, Tag>
where
    R: ResourceProvider<T, Tag>,
    T: 'static,
    Tag: 'static,
{
    type Item<'resources> = ResMut<'resources, T, Tag>;

    const ACCESS: ResourceAccess = ResourceAccess::exclusive(R::ID);

    unsafe fn fetch<'resources>(
        resources: *mut R,
    ) -> Result<Self::Item<'resources>, crate::FetchError> {
        // SAFETY: delegated to the caller and `ResourceProvider` contract.
        let value = unsafe { R::get(resources) };

        if value.is_null() || !value.is_aligned() {
            return Err(crate::FetchError::InvalidPointer);
        }

        // SAFETY: the provider guarantees a live, aligned pointer, and access
        // validation has ruled out every overlapping reference.
        Ok(ResMut {
            value: unsafe { &mut *value },
            marker: PhantomData,
        })
    }
}

pub(crate) trait ParamSet<R> {
    type Item<'resources>
    where
        R: 'resources;

    /// Validates and fetches one complete set of simultaneously live params.
    fn fetch<'resources>(
        resources: &'resources mut R,
    ) -> Result<Self::Item<'resources>, ParamError>;
}

macro_rules! impl_param_set {
    ($($param:ident => $value:ident @ $index:expr),+ $(,)?) => {
        impl<R, $($param),+> ParamSet<R> for ($($param,)+)
        where
            $($param: SystemParam<R>,)+
        {
            type Item<'resources> = ($($param::Item<'resources>,)+)
            where
                R: 'resources;

            fn fetch<'resources>(
                resources: &'resources mut R,
            ) -> Result<Self::Item<'resources>, ParamError> {
                let accesses = [$($param::ACCESS),+];
                validate_accesses(&accesses)?;
                let resources = resources as *mut R;

                $(
                    // SAFETY: the pointer comes from the exclusive borrow for
                    // `'resources`. All simultaneous accesses were validated
                    // before this first parameter was fetched.
                    let $value = unsafe { $param::fetch::<'resources>(resources) }
                        .map_err(|error| ParamError::Fetch {
                            index: $index,
                            error,
                        })?;
                )+

                Ok(($($value,)+))
            }
        }
    };
}

impl_param_set!(P0 => p0 @ 0);
impl_param_set!(P0 => p0 @ 0, P1 => p1 @ 1);
impl_param_set!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2);
impl_param_set!(P0 => p0 @ 0, P1 => p1 @ 1, P2 => p2 @ 2, P3 => p3 @ 3);
impl_param_set!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
);
impl_param_set!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
);
impl_param_set!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
);
impl_param_set!(
    P0 => p0 @ 0,
    P1 => p1 @ 1,
    P2 => p2 @ 2,
    P3 => p3 @ 3,
    P4 => p4 @ 4,
    P5 => p5 @ 5,
    P6 => p6 @ 6,
    P7 => p7 @ 7,
);

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
