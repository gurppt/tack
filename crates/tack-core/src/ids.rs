use std::{error::Error, fmt, num::NonZeroU128};

/// Zero is reserved for absent/invalid identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidId;
impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a stable identity must be nonzero")
    }
}
impl Error for InvalidId {}

macro_rules! stable_id {
    ($name:ident) => {
        #[doc = "Stable opaque 128-bit identity, supplied by the caller; never a slot or path."]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU128);
        impl $name {
            pub fn new(value: u128) -> Result<Self, InvalidId> {
                NonZeroU128::new(value).map(Self).ok_or(InvalidId)
            }
            pub fn value(self) -> u128 {
                self.0.get()
            }
        }
    };
}
stable_id!(DocumentId);
stable_id!(ObjectId);
stable_id!(AssetId);
stable_id!(SourceId);
stable_id!(GroupId);
stable_id!(BookmarkId);

/// Identity domains cannot be passed interchangeably.
/// ```compile_fail
/// use tack_core::{AssetId, ObjectId};
/// fn object(_: ObjectId) {}
/// object(AssetId::new(1).unwrap());
/// ```
/// ```compile_fail
/// use tack_core::{AssetId, SourceId};
/// fn asset(_: AssetId) {}
/// asset(SourceId::new(1).unwrap());
/// ```
/// ```compile_fail
/// use tack_core::{DocumentId, ObjectId};
/// fn document(_: DocumentId) {}
/// document(ObjectId::new(1).unwrap());
/// ```
const _: () = ();
