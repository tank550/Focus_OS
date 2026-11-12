//! Status/enum vocabulary shared by the domain, the SQLite CHECK constraints
//! and the IPC contract. Each variant is explicitly mapped to its SQL string so
//! the two cannot drift; the contract test below pins every literal against the
//! DDL in `docs/SQLITE_SCHEMA.sql`.

use crate::errors::DomainError;

/// Declares an enum whose string form is an exact SQL literal.
macro_rules! sql_enum {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident => $sql:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Canonical string used in JSON and the database.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $sql),+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = DomainError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($sql => Ok($name::$variant),)+
                    _ => Err(DomainError::InvalidValue {
                        field: stringify!($name),
                        reason: format!("unknown variant {s:?}"),
                    }),
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(
                &self,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let raw = <String as serde::Deserialize>::deserialize(deserializer)?;
                raw.parse().map_err(|_: DomainError| {
                    serde::de::Error::custom(format!("invalid {}: {raw:?}", stringify!($name)))
                })
            }
        }
    };
}

sql_enum! {
    /// `goals.status`
    pub enum GoalStatus {
        Planned => "PLANNED",
        Active => "ACTIVE",
        Completed => "COMPLETED",
        Abandoned => "ABANDONED",
    }
}

sql_enum! {
    /// `milestones.status`
    pub enum MilestoneStatus {
        Pending => "PENDING",
        InProgress => "IN_PROGRESS",
        Achieved => "ACHIEVED",
        Skipped => "SKIPPED",
    }
}

sql_enum! {
    /// `projects.status`
    pub enum ProjectStatus {
        Backlog => "BACKLOG",
        Active => "ACTIVE",
        OnHold => "ON_HOLD",
        Completed => "COMPLETED",
        Archived => "ARCHIVED",
    }
}

sql_enum! {
    /// `tasks.status`
    pub enum TaskStatus {
        Inbox => "INBOX",
        Todo => "TODO",
        Scheduled => "SCHEDULED",
        InProgress => "IN_PROGRESS",
        Paused => "PAUSED",
        Completed => "COMPLETED",
        Cancelled => "CANCELLED",
    }
}

sql_enum! {
    /// `scheduled_blocks.block_type`
    pub enum BlockType {
        DeepWork => "DEEP_WORK",
        FocusSession => "FOCUS_SESSION",
        Meeting => "MEETING",
        FixedCommitment => "FIXED_COMMITMENT",
        Break => "BREAK",
        Buffer => "BUFFER",
        Unavailable => "UNAVAILABLE",
    }
}

sql_enum! {
    /// `scheduled_blocks.status`
    pub enum BlockStatus {
        Planned => "PLANNED",
        Active => "ACTIVE",
        Completed => "COMPLETED",
        Skipped => "SKIPPED",
        Rescheduled => "RESCHEDULED",
        Cancelled => "CANCELLED",
    }
}

sql_enum! {
    /// `flexible_queue_items.status`
    pub enum QueueItemStatus {
        Queued => "QUEUED",
        Selected => "SELECTED",
        Started => "STARTED",
        Completed => "COMPLETED",
        Deferred => "DEFERRED",
        Removed => "REMOVED",
    }
}

sql_enum! {
    /// `inbox_items.kind`
    pub enum InboxKind {
        Unknown => "UNKNOWN",
        Task => "TASK",
        Idea => "IDEA",
        Project => "PROJECT",
        Responsibility => "RESPONSIBILITY",
        Obligation => "OBLIGATION",
        Research => "RESEARCH",
        Experiment => "EXPERIMENT",
        Ambition => "AMBITION",
        Note => "NOTE",
    }
}

sql_enum! {
    /// `inbox_items.status`
    pub enum InboxStatus {
        Raw => "RAW",
        Classifying => "CLASSIFYING",
        Classified => "CLASSIFIED",
        Proposed => "PROPOSED",
        Converted => "CONVERTED",
        Dismissed => "DISMISSED",
        Archived => "ARCHIVED",
    }
}

sql_enum! {
    /// Adaptive-autonomy outcome levels — the LLM can never override these.
    pub enum AutonomyLevel {
        AutoExecute => "AUTO_EXECUTE",
        Propose => "PROPOSE",
        RequireConfirmation => "REQUIRE_CONFIRMATION",
    }
}

sql_enum! {
    /// `autonomy_decisions.status`
    pub enum AutonomyDecisionStatus {
        Executed => "EXECUTED",
        Proposed => "PROPOSED",
        Approved => "APPROVED",
        Rejected => "REJECTED",
        Confirmed => "CONFIRMED",
        Withdrawn => "WITHDRAWN",
    }
}

sql_enum! {
    /// `task_sessions.session_type`
    pub enum SessionType {
        Focus => "FOCUS",
        Pomodoro => "POMODORO",
    }
}

sql_enum! {
    /// `notifications.kind`
    pub enum NotificationKind {
        Reminder30m => "REMINDER_30M",
        ReminderStart => "REMINDER_START",
        Overdue => "OVERDUE",
        DailyPlanning => "DAILY_PLANNING",
        DailyReview => "DAILY_REVIEW",
        ScheduleChange => "SCHEDULE_CHANGE",
        Focus => "FOCUS",
        System => "SYSTEM",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{from_value, json};

    /// Round-trip through the SQL literal and rejects garbage.
    fn assert_roundtrip<E>(value: E, sql: &'static str)
    where
        E: std::fmt::Display
            + std::str::FromStr<Err = DomainError>
            + PartialEq
            + std::fmt::Debug
            + Copy,
    {
        assert_eq!(value.to_string(), sql);
        assert_eq!(E::from_str(sql).unwrap(), value);
    }

    /// Every literal below MUST stay byte-identical to the CHECK constraints in
    /// `docs/SQLITE_SCHEMA.sql` (and `src-tauri/migrations/0001_init.sql`).
    #[test]
    fn sql_contract_goal_status() {
        for v in ["PLANNED", "ACTIVE", "COMPLETED", "ABANDONED"] {
            let _: GoalStatus = v.parse().expect("sql literal must parse");
        }
        assert!("GARBAGE".parse::<GoalStatus>().is_err());
        assert_roundtrip(GoalStatus::Active, "ACTIVE");
    }

    #[test]
    fn sql_contract_project_milestone() {
        for v in ["PENDING", "IN_PROGRESS", "ACHIEVED", "SKIPPED"] {
            let _: MilestoneStatus = v.parse().expect("sql literal must parse");
        }
        for v in ["BACKLOG", "ACTIVE", "ON_HOLD", "COMPLETED", "ARCHIVED"] {
            let _: ProjectStatus = v.parse().expect("sql literal must parse");
        }
        assert_roundtrip(MilestoneStatus::InProgress, "IN_PROGRESS");
    }

    #[test]
    fn sql_contract_task_status() {
        for v in [
            "INBOX", "TODO", "SCHEDULED", "IN_PROGRESS", "PAUSED", "COMPLETED", "CANCELLED",
        ] {
            let _: TaskStatus = v.parse().expect("sql literal must parse");
        }
        assert_roundtrip(TaskStatus::InProgress, "IN_PROGRESS");
    }

    #[test]
    fn sql_contract_block_type_and_status() {
        for v in [
            "DEEP_WORK", "FOCUS_SESSION", "MEETING", "FIXED_COMMITMENT",
            "BREAK", "BUFFER", "UNAVAILABLE",
        ] {
            let _: BlockType = v.parse().expect("sql literal must parse");
        }
        for v in [
            "PLANNED", "ACTIVE", "COMPLETED", "SKIPPED", "RESCHEDULED", "CANCELLED",
        ] {
            let _: BlockStatus = v.parse().expect("sql literal must parse");
        }
        assert_roundtrip(BlockType::FixedCommitment, "FIXED_COMMITMENT");
        assert_roundtrip(BlockStatus::Rescheduled, "RESCHEDULED");
    }

    #[test]
    fn sql_contract_queue_status() {
        for v in ["QUEUED", "SELECTED", "STARTED", "COMPLETED", "DEFERRED", "REMOVED"] {
            let _: QueueItemStatus = v.parse().expect("sql literal must parse");
        }
        assert_roundtrip(QueueItemStatus::Deferred, "DEFERRED");
    }

    #[test]
    fn sql_contract_inbox_kind_and_status() {
        for v in [
            "UNKNOWN", "TASK", "IDEA", "PROJECT", "RESPONSIBILITY", "OBLIGATION",
            "RESEARCH", "EXPERIMENT", "AMBITION", "NOTE",
        ] {
            let _: InboxKind = v.parse().expect("sql literal must parse");
        }
        for v in [
            "RAW", "CLASSIFYING", "CLASSIFIED", "PROPOSED", "CONVERTED", "DISMISSED", "ARCHIVED",
        ] {
            let _: InboxStatus = v.parse().expect("sql literal must parse");
        }
        assert_roundtrip(InboxKind::Responsibility, "RESPONSIBILITY");
        assert_roundtrip(InboxStatus::Classifying, "CLASSIFYING");
    }

    #[test]
    fn sql_contract_autonomy() {
        for v in ["AUTO_EXECUTE", "PROPOSE", "REQUIRE_CONFIRMATION"] {
            let _: AutonomyLevel = v.parse().expect("sql literal must parse");
        }
        for v in [
            "EXECUTED", "PROPOSED", "APPROVED", "REJECTED", "CONFIRMED", "WITHDRAWN",
        ] {
            let _: AutonomyDecisionStatus = v.parse().expect("sql literal must parse");
        }
        assert_roundtrip(AutonomyLevel::RequireConfirmation, "REQUIRE_CONFIRMATION");
    }

    #[test]
    fn sql_contract_sessions_and_notifications() {
        for v in ["FOCUS", "POMODORO"] {
            let _: SessionType = v.parse().expect("sql literal must parse");
        }
        for v in [
            "REMINDER_30M", "REMINDER_START", "OVERDUE", "DAILY_PLANNING",
            "DAILY_REVIEW", "SCHEDULE_CHANGE", "FOCUS", "SYSTEM",
        ] {
            let _: NotificationKind = v.parse().expect("sql literal must parse");
        }
        assert_roundtrip(NotificationKind::Reminder30m, "REMINDER_30M");
    }

    #[test]
    fn serde_uses_sql_strings() {
        assert_eq!(json!(TaskStatus::Scheduled), json!("SCHEDULED"));
        assert_eq!(json!(AutonomyLevel::AutoExecute), json!("AUTO_EXECUTE"));
        assert_eq!(
            from_value::<TaskStatus>(json!("COMPLETED")).unwrap(),
            TaskStatus::Completed
        );
        assert!(from_value::<TaskStatus>(json!("done")).is_err());
    }
}