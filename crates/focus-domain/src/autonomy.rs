//! Deterministic adaptive-autonomy policy (architecture §7).
//!
//! The LLM may *propose* an action, but this pure policy engine decides how it
//! is enforced: `AUTO_EXECUTE`, `PROPOSE`, or `REQUIRE_CONFIRMATION`. The rules
//! are compiled-in defaults (R01–R24) that can be overridden from settings,
//! except the hard strategic rules (R20–R24) which can only ever be tightened.
//!
//! The module has zero platform dependencies so the whole table is testable
//! headlessly.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Enforcement level for an action, ordered from least to most restrictive.
///
/// The ordering is part of the contract: an override may never move a hard
/// rule below `RequireConfirmation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutonomyLevel {
    AutoExecute,
    Propose,
    RequireConfirmation,
}

impl AutonomyLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AutoExecute => "AUTO_EXECUTE",
            Self::Propose => "PROPOSE",
            Self::RequireConfirmation => "REQUIRE_CONFIRMATION",
        }
    }
}

/// Closed set of action types the agent may request (architecture §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionType {
    MarkTaskCompleted,
    ReorderFlexibleQueue,
    InsertFlexibleTask,
    AdjustEstimate,
    RecalcRemainingCapacity,
    MoveDeepWorkBlock,
    RescheduleProjectTask,
    InsertTaskDisplacingPriority,
    SubstantialDayReplan,
    CreateTaskFromNote,
    CreateProjectFromNote,
    OrganizeAmbiguousNote,
    ChangeGoal,
    ChangeMilestone,
    DeleteTask,
    DeleteProject,
    AbandonProject,
    ChangeMandatoryDeadline,
    MoveFixedCommitment,
    ChangeQuarterlyObjective,
}

impl ActionType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarkTaskCompleted => "MARK_TASK_COMPLETED",
            Self::ReorderFlexibleQueue => "REORDER_FLEXIBLE_QUEUE",
            Self::InsertFlexibleTask => "INSERT_FLEXIBLE_TASK",
            Self::AdjustEstimate => "ADJUST_ESTIMATE",
            Self::RecalcRemainingCapacity => "RECALC_REMAINING_CAPACITY",
            Self::MoveDeepWorkBlock => "MOVE_DEEP_WORK_BLOCK",
            Self::RescheduleProjectTask => "RESCHEDULE_PROJECT_TASK",
            Self::InsertTaskDisplacingPriority => "INSERT_TASK_DISPLACING_PRIORITY",
            Self::SubstantialDayReplan => "SUBSTANTIAL_DAY_REPLAN",
            Self::CreateTaskFromNote => "CREATE_TASK_FROM_NOTE",
            Self::CreateProjectFromNote => "CREATE_PROJECT_FROM_NOTE",
            Self::OrganizeAmbiguousNote => "ORGANIZE_AMBIGUOUS_NOTE",
            Self::ChangeGoal => "CHANGE_GOAL",
            Self::ChangeMilestone => "CHANGE_MILESTONE",
            Self::DeleteTask => "DELETE_TASK",
            Self::DeleteProject => "DELETE_PROJECT",
            Self::AbandonProject => "ABANDON_PROJECT",
            Self::ChangeMandatoryDeadline => "CHANGE_MANDATORY_DEADLINE",
            Self::MoveFixedCommitment => "MOVE_FIXED_COMMITMENT",
            Self::ChangeQuarterlyObjective => "CHANGE_QUARTERLY_OBJECTIVE",
        }
    }

    /// Compiled-in fallback used only if a policy has no matching rule.
    pub const fn default_level(self) -> AutonomyLevel {
        match self {
            Self::MarkTaskCompleted
            | Self::ReorderFlexibleQueue
            | Self::InsertFlexibleTask
            | Self::AdjustEstimate
            | Self::RecalcRemainingCapacity
            | Self::CreateTaskFromNote
            | Self::CreateProjectFromNote => AutonomyLevel::AutoExecute,
            Self::MoveDeepWorkBlock
            | Self::RescheduleProjectTask
            | Self::InsertTaskDisplacingPriority
            | Self::SubstantialDayReplan
            | Self::OrganizeAmbiguousNote => AutonomyLevel::Propose,
            Self::ChangeGoal
            | Self::ChangeMilestone
            | Self::DeleteTask
            | Self::DeleteProject
            | Self::AbandonProject
            | Self::ChangeMandatoryDeadline
            | Self::MoveFixedCommitment
            | Self::ChangeQuarterlyObjective => AutonomyLevel::RequireConfirmation,
        }
    }

    /// Strategic rules whose minimum enforcement level is `RequireConfirmation`.
    ///
    /// Settings may tighten them (a no-op: they are already the strictest) but
    /// can never loosen them below explicit confirmation.
    pub const fn is_hard_strategic(self) -> bool {
        matches!(
            self,
            Self::ChangeGoal
                | Self::ChangeMilestone
                | Self::DeleteTask
                | Self::DeleteProject
                | Self::AbandonProject
                | Self::ChangeMandatoryDeadline
                | Self::MoveFixedCommitment
                | Self::ChangeQuarterlyObjective
        )
    }
}

/// Observable facts about the proposed action, evaluated by rule predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActionContext {
    /// Absolute displacement in minutes; `0` means nothing else moves.
    pub schedule_delta_minutes: u32,
    /// True when the action only adds records (never edits existing ones).
    pub additive: bool,
    /// True when the intent is uncertain or references are ambiguous.
    pub ambiguous: bool,
    pub touches_strategic: bool,
    pub touches_fixed: bool,
    pub displaces_active: bool,
    pub within_reserve: bool,
}

/// Boolean/numeric requirements a rule needs before it matches.
///
/// Every `Some(_)` predicate must hold; `None` means "not constrained".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Predicates {
    pub max_schedule_delta_minutes: Option<u32>,
    pub additive: Option<bool>,
    pub ambiguous: Option<bool>,
    pub touches_strategic: Option<bool>,
    pub touches_fixed: Option<bool>,
    pub displaces_active: Option<bool>,
    pub within_reserve: Option<bool>,
}

impl Predicates {
    pub fn matches(&self, ctx: &ActionContext) -> bool {
        if let Some(max) = self.max_schedule_delta_minutes {
            if ctx.schedule_delta_minutes > max {
                return false;
            }
        }
        let checks = [
            (self.additive, ctx.additive),
            (self.ambiguous, ctx.ambiguous),
            (self.touches_strategic, ctx.touches_strategic),
            (self.touches_fixed, ctx.touches_fixed),
            (self.displaces_active, ctx.displaces_active),
            (self.within_reserve, ctx.within_reserve),
        ];
        checks
            .into_iter()
            .all(|(expected, actual)| expected.is_none_or(|value| value == actual))
    }
}

/// One ordered policy rule. First matching rule for an action wins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    pub action_type: ActionType,
    pub level: AutonomyLevel,
    #[serde(default)]
    pub predicates: Predicates,
    pub reason: String,
}

/// Result of classifying one action against the policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Classification {
    pub action_type: ActionType,
    pub level: AutonomyLevel,
    pub rule_id: String,
    pub reason: String,
}

/// A user/application override applied over the compiled-in defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleOverride {
    pub id: String,
    pub level: AutonomyLevel,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutonomyPolicyError {
    #[error("unknown autonomy rule: {0}")]
    UnknownRule(String),
}

/// Ordered rule table plus the deterministic classifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutonomyPolicy {
    rules: Vec<PolicyRule>,
}

impl Default for AutonomyPolicy {
    fn default() -> Self {
        Self::defaults()
    }
}

impl AutonomyPolicy {
    /// Compiled-in rule table (architecture §7.1).
    pub fn defaults() -> Self {
        Self {
            rules: default_rules(),
        }
    }

    /// Apply overrides by rule id. Unknown ids are rejected; hard strategic
    /// rules are clamped to `RequireConfirmation` even if an override tries to
    /// loosen them.
    pub fn with_overrides(overrides: &[RuleOverride]) -> Result<Self, AutonomyPolicyError> {
        let mut policy = Self::defaults();
        for rule_override in overrides {
            let mut matched = false;
            for rule in policy
                .rules
                .iter_mut()
                .filter(|rule| rule.id == rule_override.id)
            {
                matched = true;
                rule.level = if rule.action_type.is_hard_strategic() {
                    AutonomyLevel::RequireConfirmation
                } else {
                    rule_override.level
                };
            }
            if !matched {
                return Err(AutonomyPolicyError::UnknownRule(rule_override.id.clone()));
            }
        }
        Ok(policy)
    }

    pub fn rules(&self) -> &[PolicyRule] {
        &self.rules
    }

    /// First matching rule wins; falls back to the action's compiled-in level.
    pub fn classify(&self, action: ActionType, ctx: &ActionContext) -> Classification {
        for rule in &self.rules {
            if rule.action_type == action && rule.predicates.matches(ctx) {
                return Classification {
                    action_type: action,
                    level: rule.level,
                    rule_id: rule.id.clone(),
                    reason: rule.reason.clone(),
                };
            }
        }
        Classification {
            action_type: action,
            level: action.default_level(),
            rule_id: "DEFAULT".to_string(),
            reason: "No rule matched; using the compiled-in default level.".to_string(),
        }
    }
}

fn rule(
    id: &str,
    action_type: ActionType,
    level: AutonomyLevel,
    predicates: Predicates,
    reason: &str,
) -> PolicyRule {
    PolicyRule {
        id: id.to_string(),
        action_type,
        level,
        predicates,
        reason: reason.to_string(),
    }
}

fn default_rules() -> Vec<PolicyRule> {
    use ActionType::*;
    use AutonomyLevel::*;

    let mut rules = vec![
        rule(
            "R01",
            MarkTaskCompleted,
            AutoExecute,
            Predicates::default(),
            "Completing a task is an explicit user action.",
        ),
        rule(
            "R02",
            ReorderFlexibleQueue,
            AutoExecute,
            Predicates::default(),
            "Reordering the flexible queue does not move a time block.",
        ),
        rule(
            "R03",
            InsertFlexibleTask,
            AutoExecute,
            Predicates {
                within_reserve: Some(true),
                displaces_active: Some(false),
                max_schedule_delta_minutes: Some(0),
                ..Predicates::default()
            },
            "A flexible task that fits free capacity without displacing anything executes automatically.",
        ),
        rule(
            "R03",
            InsertFlexibleTask,
            Propose,
            Predicates::default(),
            "A flexible task that does not clearly fit free capacity is proposed for review.",
        ),
        rule(
            "R04",
            AdjustEstimate,
            AutoExecute,
            Predicates::default(),
            "Estimate recalibration from approved execution history is low risk.",
        ),
        rule(
            "R05",
            RecalcRemainingCapacity,
            AutoExecute,
            Predicates::default(),
            "Capacity recomputation is a derived, read-only adjustment.",
        ),
        rule(
            "R10",
            MoveDeepWorkBlock,
            Propose,
            Predicates::default(),
            "Moving a committed deep-work block requires review.",
        ),
        rule(
            "R11",
            RescheduleProjectTask,
            Propose,
            Predicates::default(),
            "Non-trivial project task rescheduling requires review.",
        ),
        rule(
            "R12",
            InsertTaskDisplacingPriority,
            Propose,
            Predicates::default(),
            "Inserting work that displaces a priority block requires review.",
        ),
        rule(
            "R13",
            SubstantialDayReplan,
            Propose,
            Predicates::default(),
            "Substantial day replanning requires review.",
        ),
        rule(
            "R14",
            CreateTaskFromNote,
            AutoExecute,
            Predicates {
                additive: Some(true),
                ambiguous: Some(false),
                touches_strategic: Some(false),
                touches_fixed: Some(false),
                ..Predicates::default()
            },
            "A clear, additive task created from a note executes automatically and is audited.",
        ),
        rule(
            "R14",
            CreateTaskFromNote,
            Propose,
            Predicates::default(),
            "An uncertain or strategic note needs review before creating a task.",
        ),
        rule(
            "R15",
            CreateProjectFromNote,
            AutoExecute,
            Predicates {
                additive: Some(true),
                ambiguous: Some(false),
                touches_strategic: Some(false),
                touches_fixed: Some(false),
                ..Predicates::default()
            },
            "A clear, additive project created from a note executes automatically and is audited.",
        ),
        rule(
            "R15",
            CreateProjectFromNote,
            Propose,
            Predicates::default(),
            "An uncertain or strategic note needs review before creating a project.",
        ),
        rule(
            "R16",
            OrganizeAmbiguousNote,
            Propose,
            Predicates::default(),
            "Ambiguous classification or relationships are surfaced for review.",
        ),
    ];

    for (id, action_type, reason) in [
        (
            "R20",
            ChangeGoal,
            "Changing a goal requires explicit confirmation.",
        ),
        (
            "R20",
            ChangeMilestone,
            "Changing a milestone requires explicit confirmation.",
        ),
        (
            "R21",
            DeleteTask,
            "Deleting a task requires explicit confirmation.",
        ),
        (
            "R21",
            DeleteProject,
            "Deleting a project requires explicit confirmation.",
        ),
        (
            "R21",
            AbandonProject,
            "Abandoning a project requires explicit confirmation.",
        ),
        (
            "R22",
            ChangeMandatoryDeadline,
            "Changing a mandatory deadline requires explicit confirmation.",
        ),
        (
            "R23",
            MoveFixedCommitment,
            "Moving a fixed commitment requires explicit confirmation.",
        ),
        (
            "R24",
            ChangeQuarterlyObjective,
            "Changing a quarterly objective requires explicit confirmation.",
        ),
    ] {
        rules.push(rule(
            id,
            action_type,
            RequireConfirmation,
            Predicates::default(),
            reason,
        ));
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> ActionContext {
        ActionContext {
            additive: true,
            within_reserve: true,
            ..ActionContext::default()
        }
    }

    #[test]
    fn default_levels_match_the_policy_table() {
        let policy = AutonomyPolicy::defaults();
        let cases = [
            (ActionType::MarkTaskCompleted, AutonomyLevel::AutoExecute),
            (ActionType::ReorderFlexibleQueue, AutonomyLevel::AutoExecute),
            (ActionType::AdjustEstimate, AutonomyLevel::AutoExecute),
            (
                ActionType::RecalcRemainingCapacity,
                AutonomyLevel::AutoExecute,
            ),
            (ActionType::MoveDeepWorkBlock, AutonomyLevel::Propose),
            (ActionType::RescheduleProjectTask, AutonomyLevel::Propose),
            (
                ActionType::InsertTaskDisplacingPriority,
                AutonomyLevel::Propose,
            ),
            (ActionType::SubstantialDayReplan, AutonomyLevel::Propose),
            (ActionType::OrganizeAmbiguousNote, AutonomyLevel::Propose),
            (ActionType::ChangeGoal, AutonomyLevel::RequireConfirmation),
            (
                ActionType::ChangeMilestone,
                AutonomyLevel::RequireConfirmation,
            ),
            (ActionType::DeleteTask, AutonomyLevel::RequireConfirmation),
            (
                ActionType::DeleteProject,
                AutonomyLevel::RequireConfirmation,
            ),
            (
                ActionType::AbandonProject,
                AutonomyLevel::RequireConfirmation,
            ),
            (
                ActionType::ChangeMandatoryDeadline,
                AutonomyLevel::RequireConfirmation,
            ),
            (
                ActionType::MoveFixedCommitment,
                AutonomyLevel::RequireConfirmation,
            ),
            (
                ActionType::ChangeQuarterlyObjective,
                AutonomyLevel::RequireConfirmation,
            ),
        ];
        for (action, expected) in cases {
            let classification = policy.classify(action, &ActionContext::default());
            assert_eq!(classification.level, expected, "action {action:?}");
        }
    }

    #[test]
    fn clear_additive_note_creation_executes_automatically() {
        let policy = AutonomyPolicy::defaults();
        let task = policy.classify(ActionType::CreateTaskFromNote, &context());
        assert_eq!(task.level, AutonomyLevel::AutoExecute);
        assert_eq!(task.rule_id, "R14");

        let project = policy.classify(ActionType::CreateProjectFromNote, &context());
        assert_eq!(project.level, AutonomyLevel::AutoExecute);
        assert_eq!(project.rule_id, "R15");
    }

    #[test]
    fn ambiguous_or_strategic_note_creation_is_proposed() {
        let policy = AutonomyPolicy::defaults();

        let ambiguous = policy.classify(
            ActionType::CreateTaskFromNote,
            &ActionContext {
                ambiguous: true,
                ..context()
            },
        );
        assert_eq!(ambiguous.level, AutonomyLevel::Propose);

        let strategic = policy.classify(
            ActionType::CreateProjectFromNote,
            &ActionContext {
                touches_strategic: true,
                ..context()
            },
        );
        assert_eq!(strategic.level, AutonomyLevel::Propose);
    }

    #[test]
    fn flexible_task_that_displaces_work_is_proposed() {
        let policy = AutonomyPolicy::defaults();
        let classification = policy.classify(
            ActionType::InsertFlexibleTask,
            &ActionContext {
                within_reserve: true,
                displaces_active: true,
                schedule_delta_minutes: 30,
                ..ActionContext::default()
            },
        );
        assert_eq!(classification.level, AutonomyLevel::Propose);
    }

    #[test]
    fn overrides_can_loosen_and_tighten_soft_rules() {
        let policy = AutonomyPolicy::with_overrides(&[
            RuleOverride {
                id: "R10".into(),
                level: AutonomyLevel::AutoExecute,
            },
            RuleOverride {
                id: "R01".into(),
                level: AutonomyLevel::RequireConfirmation,
            },
        ])
        .unwrap();

        assert_eq!(
            policy
                .classify(ActionType::MoveDeepWorkBlock, &ActionContext::default())
                .level,
            AutonomyLevel::AutoExecute
        );
        assert_eq!(
            policy
                .classify(ActionType::MarkTaskCompleted, &ActionContext::default())
                .level,
            AutonomyLevel::RequireConfirmation
        );
    }

    #[test]
    fn hard_strategic_rules_can_never_be_loosened() {
        let policy = AutonomyPolicy::with_overrides(&[RuleOverride {
            id: "R20".into(),
            level: AutonomyLevel::AutoExecute,
        }])
        .unwrap();

        for action in [ActionType::ChangeGoal, ActionType::ChangeMilestone] {
            assert_eq!(
                policy.classify(action, &ActionContext::default()).level,
                AutonomyLevel::RequireConfirmation,
                "action {action:?} must stay confirmed"
            );
        }
    }

    #[test]
    fn unknown_override_is_rejected() {
        let error = AutonomyPolicy::with_overrides(&[RuleOverride {
            id: "R99".into(),
            level: AutonomyLevel::AutoExecute,
        }])
        .unwrap_err();
        assert_eq!(error, AutonomyPolicyError::UnknownRule("R99".into()));
    }

    #[test]
    fn first_matching_rule_wins() {
        let policy = AutonomyPolicy::defaults();
        let auto = policy.classify(
            ActionType::InsertFlexibleTask,
            &ActionContext {
                within_reserve: true,
                ..ActionContext::default()
            },
        );
        assert_eq!(auto.level, AutonomyLevel::AutoExecute);
        assert_eq!(auto.rule_id, "R03");

        let proposed = policy.classify(
            ActionType::InsertFlexibleTask,
            &ActionContext {
                within_reserve: false,
                ..ActionContext::default()
            },
        );
        assert_eq!(proposed.level, AutonomyLevel::Propose);
    }

    #[test]
    fn classification_serializes_for_the_audit_snapshot() {
        let policy = AutonomyPolicy::defaults();
        let classification = policy.classify(ActionType::CreateTaskFromNote, &context());
        let json = serde_json::to_value(&classification).unwrap();
        assert_eq!(json["actionType"], "CREATE_TASK_FROM_NOTE");
        assert_eq!(json["level"], "AUTO_EXECUTE");
        assert_eq!(json["ruleId"], "R14");
    }
}
