//! Focus OS domain model.
//!
//! This crate is a pure Rust library: it must NOT depend on Tauri, SQLite, or
//! any platform API, so that the scheduler and autonomy policy are testable
//! headlessly (`cargo test -p focus-domain`) and cannot accidentally acquire
//! write access to the persistence layer.

pub mod errors;
pub mod ids;
pub mod status;
pub mod value_objects;

#[cfg(test)]
mod smoke {
    use super::ids::EntityId;

    #[test]
    fn entity_id_generation_and_parse() {
        let id = EntityId::new();
        assert!(!id.is_null());
        assert_eq!(id.to_string().len(), 36);
        let parsed: EntityId = id.to_string().parse().expect("roundtrip parse");
        assert_eq!(parsed, id);
    }
}