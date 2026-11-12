use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// UUIDv7 entity identifier. Time-ordered — friendly to SQLite index locality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(Uuid);

impl EntityId {
    /// Generate a fresh time-ordered identifier.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Convenience alias mirroring common `random` naming.
    pub fn random() -> Self {
        Self::new()
    }

    /// The nil UUID — used for "no reference" placeholders only.
    pub fn null() -> Self {
        Self(Uuid::nil())
    }

    pub fn is_null(&self) -> bool {
        self.0.is_nil()
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for EntityId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Serialize for EntityId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Uuid::parse_str(&raw)
            .map(Self)
            .map_err(|e| serde::de::Error::custom(format!("invalid entity id: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_value, json, to_value};

    #[test]
    fn serde_serializes_as_canonical_string() {
        let id = EntityId::new();
        assert_eq!(to_value(id).unwrap(), json!(id.to_string()));
    }

    #[test]
    fn serde_rejects_malformed_ids() {
        assert!(from_value::<EntityId>(json!("not-a-uuid")).is_err());
    }
}