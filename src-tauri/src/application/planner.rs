use serde::{Deserialize, Serialize};
use thiserror::Error;

use focus_domain::ids::EntityId;
use focus_domain::value_objects::UtcMillis;

use crate::application::settings::{self, ScheduleSettings};
use crate::infrastructure::db::DbError;
use crate::infrastructure::repositories::planner_repo::{
    DailyPlanRecord, NewDailyPlan, PlannerRepo, PlanningTaskRecord, QueueItemRecord,
    ScheduledBlockRecord,
};
use crate::infrastructure::repositories::tasks_repo::TasksRepo;
use crate::state::AppState;

const MIN_RESERVE_MINUTES: u32 = 30;
const MIN_BUFFER_MINUTES: u32 = 10;
const DEADLINE_URGENCY_WINDOW_MS: i64 = 72 * 60 * 60 * 1000;
const MILLIS_PER_MINUTE: i64 = 60_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedTask {
    pub task_id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: u16,
    pub estimated_minutes: u32,
    pub score: f64,
    pub deadline: Option<i64>,
    pub fits_capacity: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPlan {
    pub id: String,
    pub plan_date: String,
    pub version: i64,
    pub capacity_minutes: u32,
    pub planned_work_minutes: u32,
    pub overflow_minutes: u32,
    pub workload_reserve_minutes: u32,
    pub tasks: Vec<PlannedTask>,
    pub scheduled_blocks: Vec<ScheduledBlock>,
    pub blocked_tasks: Vec<PlannedTask>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledBlock {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub description: Option<String>,
    pub scheduled_start: i64,
    pub scheduled_end: i64,
    pub duration_minutes: u32,
    pub priority: u16,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeasibilitySnapshot {
    capacity_minutes: u32,
    overflow_minutes: u32,
    overflow_task_ids: Vec<String>,
    #[serde(default)]
    scheduled_task_ids: Vec<String>,
    blocked_tasks: Vec<PlannedTask>,
}

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("The plan date must be a valid YYYY-MM-DD date")]
    InvalidDate,
    #[error("The local UTC offset must be between -14 and +14 hours")]
    InvalidUtcOffset,
    #[error("Schedule settings are invalid: {0}")]
    InvalidScheduleSettings(String),
    #[error("Stored plan data is invalid: {0}")]
    InvalidStoredPlan(String),
    #[error("Could not serialize the daily plan: {0}")]
    Serialization(String),
    #[error(transparent)]
    Database(#[from] DbError),
}

pub fn today(state: &AppState, plan_date: &str) -> Result<Option<DailyPlan>, PlannerError> {
    validate_date(plan_date)?;
    let conn = state.conn()?;
    PlannerRepo::active_plan(&conn, plan_date)?
        .map(plan_from_record)
        .transpose()
}

pub fn plan_day(
    state: &AppState,
    plan_date: &str,
    utc_offset_minutes: i32,
) -> Result<DailyPlan, PlannerError> {
    validate_date(plan_date)?;
    if !(-14 * 60..=14 * 60).contains(&utc_offset_minutes) {
        return Err(PlannerError::InvalidUtcOffset);
    }
    let schedule = load_schedule_settings(state)?;
    let (capacity_minutes, reserve_minutes) = capacity(&schedule)?;
    let working_start = parse_clock(&schedule.working_hours_start)?;
    let working_end = parse_clock(&schedule.working_hours_end)?;
    let mut slot_cursor =
        epoch_minute_for_local_time(plan_date, working_start, utc_offset_minutes)?;
    let workday_end = epoch_minute_for_local_time(plan_date, working_end, utc_offset_minutes)?;
    let now = UtcMillis::now().0;

    let mut conn = state.conn()?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(DbError::from)?;
    let active_task_id = TasksRepo::active_session(&tx)?.map(|session| session.task_id);
    let active_session_blocks = PlannerRepo::active_session_blocks(&tx, plan_date)?;
    let records = PlannerRepo::planning_tasks(&tx)?;
    let (mut eligible, blocked): (Vec<_>, Vec<_>) = records
        .into_iter()
        .filter(|task| active_task_id.as_deref() != Some(task.id.as_str()))
        .partition(|task| !task.dependency_blocked);

    eligible.sort_by(|left, right| {
        task_score(right, now)
            .total_cmp(&task_score(left, now))
            .then_with(|| {
                left.deadline
                    .unwrap_or(i64::MAX)
                    .cmp(&right.deadline.unwrap_or(i64::MAX))
            })
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut planned_work_minutes = 0u32;
    let mut overflow_minutes = 0u32;
    let mut overflow_task_ids = Vec::new();
    let mut scheduled_task_ids = Vec::new();
    let mut scheduled_blocks = active_session_blocks
        .iter()
        .cloned()
        .map(|record| {
            let mut block = block_from_record(record);
            block.id = EntityId::new().to_string();
            block
        })
        .collect::<Vec<_>>();
    let mut queue_tasks = Vec::new();
    let blocked_tasks = blocked
        .iter()
        .map(|task| to_planned_task(task, now))
        .collect::<Vec<_>>();
    for task in &eligible {
        let exceeds_capacity =
            planned_work_minutes.saturating_add(task.estimated_minutes) > capacity_minutes;
        if exceeds_capacity {
            overflow_minutes = overflow_minutes.saturating_add(task.estimated_minutes);
            overflow_task_ids.push(task.id.clone());
            queue_tasks.push(task.clone());
            continue;
        }

        if task.requires_deep_work || task.estimated_minutes >= 60 {
            let duration = i64::from(task.estimated_minutes);
            let end = slot_cursor.saturating_add(duration);
            if end <= workday_end {
                scheduled_blocks.push(ScheduledBlock {
                    id: EntityId::new().to_string(),
                    task_id: task.id.clone(),
                    title: task.title.clone(),
                    description: task.description.clone(),
                    scheduled_start: slot_cursor.saturating_mul(MILLIS_PER_MINUTE),
                    scheduled_end: end.saturating_mul(MILLIS_PER_MINUTE),
                    duration_minutes: task.estimated_minutes,
                    priority: task.priority,
                    active: false,
                });
                slot_cursor = end.saturating_add(i64::from(MIN_BUFFER_MINUTES));
                planned_work_minutes += task.estimated_minutes;
                scheduled_task_ids.push(task.id.clone());
            } else {
                overflow_minutes = overflow_minutes.saturating_add(task.estimated_minutes);
                overflow_task_ids.push(task.id.clone());
                queue_tasks.push(task.clone());
            }
        } else {
            planned_work_minutes += task.estimated_minutes;
            queue_tasks.push(task.clone());
        }
    }

    let snapshot = FeasibilitySnapshot {
        capacity_minutes,
        overflow_minutes,
        overflow_task_ids: overflow_task_ids.clone(),
        scheduled_task_ids,
        blocked_tasks: blocked_tasks.clone(),
    };
    let feasibility = serde_json::to_string(&snapshot)
        .map_err(|error| PlannerError::Serialization(error.to_string()))?;
    let plan_id = EntityId::new().to_string();
    let version = PlannerRepo::next_version(&tx, plan_date)?;
    let previous_id = PlannerRepo::commit_plan(
        &tx,
        &NewDailyPlan {
            id: &plan_id,
            plan_date,
            version,
            planned_work_minutes,
            reserve_minutes,
            feasibility: &feasibility,
            created_at: now,
        },
    )?;

    for (position, task) in queue_tasks.iter().enumerate() {
        let urgency = deadline_urgency(task.deadline, now);
        PlannerRepo::insert_queue_item(
            &tx,
            &plan_id,
            position + 1,
            task,
            task_score(task, now),
            urgency,
            now,
        )?;
    }

    for (task, block) in eligible.iter().filter_map(|task| {
        scheduled_blocks
            .iter()
            .find(|block| block.task_id == task.id && !block.active)
            .map(|block| (task, block))
    }) {
        PlannerRepo::insert_scheduled_block(
            &tx,
            &plan_id,
            task,
            &block.id,
            block.scheduled_start,
            block.scheduled_end,
            now,
        )?;
    }

    for (source_block, copied_block) in active_session_blocks.iter().filter_map(|source| {
        scheduled_blocks
            .iter()
            .find(|copy| copy.task_id == source.task_id && copy.active)
            .map(|copy| (source, copy))
    }) {
        PlannerRepo::copy_active_block(&tx, &source_block.id, &plan_id, &copied_block.id, now)?;
    }

    let event_id = EntityId::new().to_string();
    let after_payload = serde_json::to_string(&snapshot)
        .map_err(|error| PlannerError::Serialization(error.to_string()))?;
    let undo_payload = serde_json::json!({
        "operation": "restore_previous_active_plan",
        "previousPlanId": previous_id,
        "createdPlanId": plan_id,
    })
    .to_string();
    PlannerRepo::insert_plan_event(
        &tx,
        &event_id,
        &plan_id,
        now,
        previous_id.as_deref(),
        &after_payload,
        &undo_payload,
    )?;
    tx.commit().map_err(DbError::from)?;

    Ok(DailyPlan {
        id: plan_id,
        plan_date: plan_date.to_owned(),
        version,
        capacity_minutes,
        planned_work_minutes,
        overflow_minutes,
        workload_reserve_minutes: reserve_minutes,
        tasks: queue_tasks
            .iter()
            .map(|task| {
                let mut planned = to_planned_task(task, now);
                planned.score = task_score(task, now);
                planned.fits_capacity = !overflow_task_ids.contains(&task.id);
                planned
            })
            .collect(),
        scheduled_blocks,
        blocked_tasks,
        created_at: now,
    })
}

fn plan_from_record(record: DailyPlanRecord) -> Result<DailyPlan, PlannerError> {
    let feasibility = record
        .feasibility
        .ok_or_else(|| PlannerError::InvalidStoredPlan("missing feasibility data".into()))?;
    let snapshot: FeasibilitySnapshot = serde_json::from_str(&feasibility)
        .map_err(|error| PlannerError::InvalidStoredPlan(error.to_string()))?;
    let overflow_ids = snapshot
        .overflow_task_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let tasks = record
        .items
        .into_iter()
        .map(|item| {
            let fits_capacity = !overflow_ids.contains(item.task_id.as_str());
            planned_from_queue(item, fits_capacity)
        })
        .collect();
    let scheduled_blocks = record.blocks.into_iter().map(block_from_record).collect();
    Ok(DailyPlan {
        id: record.id,
        plan_date: record.plan_date,
        version: record.version,
        capacity_minutes: snapshot.capacity_minutes,
        planned_work_minutes: record.planned_work_minutes,
        overflow_minutes: snapshot.overflow_minutes,
        workload_reserve_minutes: record.workload_reserve_minutes,
        tasks,
        scheduled_blocks,
        blocked_tasks: snapshot.blocked_tasks,
        created_at: record.created_at,
    })
}

fn block_from_record(block: ScheduledBlockRecord) -> ScheduledBlock {
    ScheduledBlock {
        id: block.id,
        task_id: block.task_id,
        title: block.title,
        description: block.description,
        scheduled_start: block.scheduled_start,
        scheduled_end: block.scheduled_end,
        duration_minutes: block.duration_minutes,
        priority: block.priority,
        active: block.status == "ACTIVE",
    }
}

fn planned_from_queue(item: QueueItemRecord, fits_capacity: bool) -> PlannedTask {
    PlannedTask {
        task_id: item.task_id,
        title: item.title,
        description: item.description,
        priority: item.priority,
        estimated_minutes: item.estimated_minutes,
        score: item.score,
        deadline: item.deadline,
        fits_capacity,
    }
}

fn to_planned_task(task: &PlanningTaskRecord, now: i64) -> PlannedTask {
    PlannedTask {
        task_id: task.id.clone(),
        title: task.title.clone(),
        description: task.description.clone(),
        priority: task.priority,
        estimated_minutes: task.estimated_minutes,
        score: task_score(task, now),
        deadline: task.deadline,
        fits_capacity: false,
    }
}

fn task_score(task: &PlanningTaskRecord, now: i64) -> f64 {
    let priority = f64::from(task.priority.min(100)) / 100.0;
    let urgency = f64::from(deadline_urgency(task.deadline, now)) / 100.0;
    let effort = 1.0 - (f64::from(task.estimated_minutes.min(240)) / 240.0);
    let postponement_bonus = (f64::from(task.postponement_count) * 0.03).min(0.15);
    (priority * 0.65 + urgency * 0.25 + effort * 0.10 + postponement_bonus).clamp(0.0, 1.0)
}

fn deadline_urgency(deadline: Option<i64>, now: i64) -> u16 {
    let Some(deadline) = deadline else {
        return 0;
    };
    if deadline <= now {
        return 100;
    }
    let remaining = deadline - now;
    if remaining >= DEADLINE_URGENCY_WINDOW_MS {
        0
    } else {
        ((DEADLINE_URGENCY_WINDOW_MS - remaining) as f64 / DEADLINE_URGENCY_WINDOW_MS as f64
            * 100.0)
            .round() as u16
    }
}

fn load_schedule_settings(state: &AppState) -> Result<ScheduleSettings, PlannerError> {
    settings::get_raw(state, settings::KEY_SCHEDULE)?
        .map(|raw| {
            serde_json::from_str::<ScheduleSettings>(&raw)
                .map_err(|error| PlannerError::InvalidScheduleSettings(error.to_string()))
        })
        .transpose()
        .map(|settings| settings.unwrap_or_default())
}

fn capacity(settings: &ScheduleSettings) -> Result<(u32, u32), PlannerError> {
    let start = parse_clock(&settings.working_hours_start)?;
    let end = parse_clock(&settings.working_hours_end)?;
    if end <= start {
        return Err(PlannerError::InvalidScheduleSettings(
            "working hours must end after they start".into(),
        ));
    }
    if settings.max_daily_minutes == 0
        || settings.max_daily_minutes > 24 * 60
        || !settings.reserve_ratio.is_finite()
        || !(0.0..=1.0).contains(&settings.reserve_ratio)
    {
        return Err(PlannerError::InvalidScheduleSettings(
            "daily limit must be 1..1440 minutes and reserve ratio 0..1".into(),
        ));
    }
    let working_minutes = end - start;
    let total_capacity = settings.max_daily_minutes.min(working_minutes);
    let reserve_minutes = (total_capacity as f64 * settings.reserve_ratio)
        .round()
        .max(f64::from(MIN_RESERVE_MINUTES))
        .min(f64::from(total_capacity)) as u32;
    Ok((total_capacity - reserve_minutes, reserve_minutes))
}

fn parse_clock(value: &str) -> Result<u32, PlannerError> {
    let invalid = || PlannerError::InvalidScheduleSettings(format!("invalid time value: {value}"));
    let (hour, minute) = value.split_once(':').ok_or_else(invalid)?;
    if hour.len() != 2 || minute.len() != 2 {
        return Err(invalid());
    }
    let hour = hour.parse::<u32>().map_err(|_| invalid())?;
    let minute = minute.parse::<u32>().map_err(|_| invalid())?;
    if hour > 23 || minute > 59 {
        return Err(invalid());
    }
    Ok(hour * 60 + minute)
}

fn epoch_minute_for_local_time(
    date: &str,
    local_minute: u32,
    utc_offset_minutes: i32,
) -> Result<i64, PlannerError> {
    let days = days_from_civil(date)?;
    Ok(days
        .saturating_mul(24 * 60)
        .saturating_add(i64::from(local_minute))
        .saturating_add(i64::from(utc_offset_minutes)))
}

fn days_from_civil(value: &str) -> Result<i64, PlannerError> {
    let year = value[0..4]
        .parse::<i64>()
        .map_err(|_| PlannerError::InvalidDate)?;
    let month = value[5..7]
        .parse::<i64>()
        .map_err(|_| PlannerError::InvalidDate)?;
    let day = value[8..10]
        .parse::<i64>()
        .map_err(|_| PlannerError::InvalidDate)?;
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Ok(era * 146_097 + day_of_era - 719_468)
}

fn validate_date(value: &str) -> Result<(), PlannerError> {
    let invalid = || PlannerError::InvalidDate;
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return Err(invalid());
    }

    let year = parts[0].parse::<u32>().map_err(|_| invalid())?;
    let month = parts[1].parse::<u32>().map_err(|_| invalid())?;
    let day = parts[2].parse::<u32>().map_err(|_| invalid())?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(invalid()),
    };
    if day == 0 || day > days {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::repositories::tasks_repo::{TaskRecord, TasksRepo};

    fn task(
        id: &str,
        priority: u16,
        deadline: Option<i64>,
        estimated_minutes: u32,
    ) -> PlanningTaskRecord {
        PlanningTaskRecord {
            id: id.into(),
            title: id.into(),
            description: None,
            priority,
            estimated_minutes,
            requires_deep_work: false,
            deadline,
            dependency_blocked: false,
            context_tag: None,
            energy_level: None,
            postponement_count: 0,
            created_at: 1,
        }
    }

    #[test]
    fn urgency_rises_within_window_and_is_capped_for_overdue_tasks() {
        let now = 1_000_000;
        assert_eq!(deadline_urgency(None, now), 0);
        assert_eq!(
            deadline_urgency(Some(now + DEADLINE_URGENCY_WINDOW_MS), now),
            0
        );
        assert_eq!(
            deadline_urgency(Some(now + DEADLINE_URGENCY_WINDOW_MS / 2), now),
            50
        );
        assert_eq!(deadline_urgency(Some(now - 1), now), 100);
    }

    #[test]
    fn capacity_honors_working_hours_daily_limit_and_minimum_reserve() {
        let settings = ScheduleSettings {
            working_hours_start: "09:00".into(),
            working_hours_end: "17:00".into(),
            max_daily_minutes: 480,
            reserve_ratio: 0.15,
        };
        assert_eq!(capacity(&settings).unwrap(), (408, 72));
    }

    #[test]
    fn rejects_invalid_and_non_calendar_dates() {
        assert!(validate_date("2026-02-29").is_err());
        assert!(validate_date("2028-02-29").is_ok());
        assert!(validate_date("2026-13-01").is_err());
    }

    #[test]
    fn converts_local_working_hours_to_utc_using_the_browser_offset() {
        assert_eq!(
            epoch_minute_for_local_time("1970-01-01", 540, 0).unwrap(),
            540
        );
        assert_eq!(
            epoch_minute_for_local_time("1970-01-01", 540, -60).unwrap(),
            480
        );
        assert_eq!(
            epoch_minute_for_local_time("1970-01-01", 540, 330).unwrap(),
            870
        );
    }

    #[test]
    fn ranking_uses_priority_and_deadline_and_preserves_a_stable_tie_break() {
        let now = 1_000_000;
        let mut tasks = [
            task("later", 50, None, 30),
            task("urgent", 20, Some(now + 1), 30),
            task("priority", 90, None, 30),
            task("tie-b", 50, None, 30),
            task("tie-a", 50, None, 30),
        ];
        tasks.sort_by(|left, right| {
            task_score(right, now)
                .total_cmp(&task_score(left, now))
                .then_with(|| {
                    left.deadline
                        .unwrap_or(i64::MAX)
                        .cmp(&right.deadline.unwrap_or(i64::MAX))
                })
                .then_with(|| left.created_at.cmp(&right.created_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        assert_eq!(
            tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            vec!["priority", "urgent", "later", "tie-a", "tie-b"]
        );
    }

    #[test]
    fn persists_ranked_queue_overflow_and_dependency_blockers_with_versions() {
        let directory = std::env::temp_dir().join(format!("focus-os-planner-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        let state = AppState::new(pool, database_path);
        {
            let conn = state.conn().unwrap();
            for (id, title, priority, estimated_minutes) in [
                ("long", "High priority", 90, 240),
                ("medium", "Next in queue", 80, 150),
                ("overflow", "Beyond capacity", 70, 180),
                ("predecessor", "Dependency predecessor", 10, 25),
                ("blocked", "Wait for predecessor", 100, 25),
            ] {
                TasksRepo::insert_from_agent(
                    &conn,
                    &TaskRecord {
                        id: id.into(),
                        project_id: None,
                        goal_id: None,
                        title: title.into(),
                        description: None,
                        status: "TODO".into(),
                        priority,
                        estimated_minutes,
                        actual_minutes: 0,
                        requires_deep_work: false,
                        created_at: 1,
                        updated_at: 1,
                    },
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO task_dependencies
                    (id, task_id, depends_on_task_id, dependency_type)
                 VALUES ('dependency', 'blocked', 'predecessor', 'FINISH_TO_START')",
                [],
            )
            .unwrap();
        }

        let first = plan_day(&state, "2026-11-13", 0).unwrap();
        assert_eq!(first.version, 1);
        assert_eq!(first.capacity_minutes, 408);
        assert_eq!(first.planned_work_minutes, 390);
        assert_eq!(first.overflow_minutes, 205);
        assert_eq!(
            first
                .tasks
                .iter()
                .map(|task| task.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["overflow", "predecessor"]
        );
        assert_eq!(
            first
                .scheduled_blocks
                .iter()
                .map(|block| block.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["long", "medium"]
        );
        assert_eq!(
            first.scheduled_blocks[0].scheduled_start,
            epoch_minute_for_local_time("2026-11-13", 9 * 60, 0).unwrap() * MILLIS_PER_MINUTE
        );
        assert_eq!(
            first.scheduled_blocks[1].scheduled_start - first.scheduled_blocks[0].scheduled_end,
            i64::from(MIN_BUFFER_MINUTES) * MILLIS_PER_MINUTE
        );
        assert!(!first.tasks[0].fits_capacity);
        assert_eq!(first.blocked_tasks[0].task_id, "blocked");
        assert_eq!(today(&state, "2026-11-13").unwrap(), Some(first.clone()));

        let second = plan_day(&state, "2026-11-13", 0).unwrap();
        assert_eq!(second.version, 2);
        let conn = state.conn().unwrap();
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schedule_events WHERE event_type = 'PLAN_REPLANNED'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 2);
        let superseded_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM daily_plans WHERE plan_date = '2026-11-13' AND status = 'SUPERSEDED'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(superseded_count, 1);
        drop(conn);

        crate::application::tasks::start_focus(&state, "long").unwrap();
        let frozen_start = second.scheduled_blocks[0].scheduled_start;
        let replanned = plan_day(&state, "2026-11-13", 0).unwrap();
        assert!(replanned.scheduled_blocks.iter().any(|block| {
            block.task_id == "long" && block.scheduled_start == frozen_start && block.active
        }));
        let started_plan = today(&state, "2026-11-13").unwrap().unwrap();
        assert!(started_plan
            .scheduled_blocks
            .iter()
            .any(|block| block.task_id == "long" && block.scheduled_start == frozen_start));
        crate::application::tasks::pause_focus(&state, "long").unwrap();
        assert!(today(&state, "2026-11-13")
            .unwrap()
            .unwrap()
            .scheduled_blocks
            .iter()
            .any(|block| block.task_id == "long"));
        crate::application::tasks::complete(&state, "long").unwrap();
        let completed_plan = today(&state, "2026-11-13").unwrap().unwrap();
        assert!(!completed_plan
            .scheduled_blocks
            .iter()
            .any(|block| block.task_id == "long"));
        assert!(!completed_plan
            .tasks
            .iter()
            .any(|task| task.task_id == "long"));

        drop(state);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn rejects_invalid_date_before_creating_a_plan() {
        let directory =
            std::env::temp_dir().join(format!("focus-os-planner-date-{}", EntityId::new()));
        let database_path = directory.join("test.db");
        let pool = crate::infrastructure::db::open(&database_path).unwrap();
        let state = AppState::new(pool, database_path);

        assert!(matches!(
            plan_day(&state, "2026-02-29", 0),
            Err(PlannerError::InvalidDate)
        ));
        assert!(today(&state, "not-a-date").is_err());

        drop(state);
        let _ = std::fs::remove_dir_all(directory);
    }
}
