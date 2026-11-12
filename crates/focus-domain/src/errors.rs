use thiserror::Error;

/// Typed domain-level errors. Command/application layers map these to their
/// own boundary types; no raw strings leak upward.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid status transition: {from} -> {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },

    #[error("invalid value for `{field}`: {reason}")]
    InvalidValue {
        field: &'static str,
        reason: String,
    },

    #[error("entity not found: {entity}:{id}")]
    NotFound { entity: &'static str, id: String },

    #[error("business rule violated: {0}")]
    RuleViolation(String),
}