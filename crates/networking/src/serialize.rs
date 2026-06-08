//! Message serialization helpers using `bincode`.

use serde::{Serialize, Deserialize};

/// Serialize a value to a compact binary `Vec<u8>` using `bincode`.
pub fn serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(value)
}

/// Deserialize a value from a byte slice using `bincode`.
pub fn deserialize<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(bytes)
}

/// Serialize a value to a `Vec<u8>`, panicking on error (useful for types that are
/// known to serialize successfully, like simple structs).
pub fn serialize_unchecked<T: Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).expect("bincode serialize failed")
}

/// Deserialize a value from a byte slice, panicking on error.
pub fn deserialize_unchecked<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> T {
    bincode::deserialize(bytes).expect("bincode deserialize failed")
}
