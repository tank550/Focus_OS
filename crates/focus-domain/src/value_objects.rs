use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::errors::DomainError;

/// Milliseconds since the UNIX epoch (UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UtcMillis(pub i64);

impl UtcMillis {
    pub fn now() -> Self {
        let d = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self(d.as_millis() as i64)
    }
}

/// Priority on the 0..=100 scale used across tasks, projects and goals.
/// Constructed values are clamped or rejected at the boundary — the type
/// itself guarantees the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(u16);

impl Priority {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(100);

    pub fn new(value: u16) -> Result<Self, DomainError> {
        if value > 100 {
            return Err(DomainError::InvalidValue {
                field: "priority",
                reason: format!("{value} exceeds 100"),
            });
        }
        Ok(Self(value))
    }

    pub fn clamp(value: u16) -> Self {
        Self(value.clamp(0, 100))
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

/// Normalized score in [0, 1] — the scheduler output unit.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Score(f64);

impl Score {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(DomainError::InvalidValue {
                field: "score",
                reason: format!("{value} is outside [0, 1]"),
            });
        }
        Ok(Self(value))
    }

    pub fn clamp(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// A positive work duration in whole minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Minutes(u32);

impl Minutes {
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::InvalidValue {
                field: "minutes",
                reason: "duration must be positive".to_string(),
            });
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_millis_now_is_positive_and_monotonic() {
        let a = UtcMillis::now();
        let b = UtcMillis::now();
        assert!(b.0 >= a.0);
        assert!(a.0 > 1_600_000_000_000); // > 2020
    }

    #[test]
    fn priority_range_is_enforced() {
        assert!(Priority::new(0).is_ok());
        assert!(Priority::new(100).is_ok());
        assert!(Priority::new(101).is_err());
        assert_eq!(Priority::clamp(250).get(), 100);
    }

    #[test]
    fn score_range_is_enforced() {
        assert!(Score::new(0.0).is_ok());
        assert!(Score::new(1.0).is_ok());
        assert!(Score::new(1.1).is_err());
        assert!(Score::new(-0.1).is_err());
        assert_eq!(Score::clamp(2.5).get(), 1.0);
    }

    #[test]
    fn minutes_must_be_positive() {
        assert!(Minutes::new(1).is_ok());
        assert!(Minutes::new(0).is_err());
    }
}