use std::marker::PhantomData;

/// A unique identifier for an asset type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetTypeId(pub u64);

impl AssetTypeId {
    /// Create a stable ID from a crate-specific name string.
    pub fn from_crate_name(name: &str) -> Self {
        let mut hash = 0u64;
        for (i, c) in name.bytes().enumerate() {
            hash = hash.wrapping_add((c as u64).wrapping_mul(i as u64 + 1));
        }
        Self(hash)
    }
}

/// Marker trait for types that can be stored and loaded as engine assets.
pub trait Asset: Send + Sync + 'static {
    fn asset_type_id() -> AssetTypeId where Self: Sized;
}

/// A lightweight, copyable handle to an asset stored in the registry.
///
/// 8 bytes total: 32-bit index + 32-bit generation.
/// Generation is incremented when an asset is replaced,
/// preventing stale handles from accessing wrong data.
#[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Handle<T: Asset> {
    index: u32,
    generation: u32,
    #[serde(skip)]
    _marker: PhantomData<T>,
}

// SAFETY: Handle only contains plain integers and PhantomData.
unsafe impl<T: Asset> Send for Handle<T> {}
unsafe impl<T: Asset> Sync for Handle<T> {}

impl<T: Asset> Handle<T> {
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation, _marker: PhantomData }
    }

    pub fn index(&self) -> u32 { self.index }
    pub fn generation(&self) -> u32 { self.generation }

    /// Erase the type information, producing an untyped handle.
    pub fn erase(self) -> UntypedHandle {
        UntypedHandle { index: self.index, generation: self.generation }
    }
}

impl<T: Asset> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Handle<{}>(i={}, g={})", std::any::type_name::<T>(), self.index, self.generation)
    }
}

/// A type-erased handle that can reference any asset type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UntypedHandle {
    pub index: u32,
    pub generation: u32,
}

impl UntypedHandle {
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub fn typed<T: Asset>(self) -> Handle<T> {
        Handle::new(self.index, self.generation)
    }
}