use rusqlite::{Connection, OptionalExtension};

use crate::infrastructure::db::DbError;

#[derive(Debug, Clone, PartialEq)]
pub struct PlanningTaskRecord {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: u16,
    pub estimated_minutes: u32,
    pub requires_deep_work: bool,
    pub deadline: Option<i64>,
    pub dependency_blocked: bool,
    pub context_tag: Option<String>,
    pub energy_level: Option<String>,
    pub postponement_count: u32,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueItemRecord {
    pub task_id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: u16,
    pub estimated_minutes: u32,
    pub score: f64,
    pub deadline: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledBlockRecord {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub description: Option<String>,
    pub scheduled_start: i64,
    pub scheduled_end: i64,
    pub duration_minutes: u32,
    pub priority: u16,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyPlanRecord {
    pub id: String,
    pub plan_date: String,
    pub version: i64,
    pub planned_work_minutes: u32,
    pub workload_reserve_minutes: u32,
    pub feasibility: Option<String>,
    pub created_at: i64,
    pub items: Vec<QueueItemRecord>,
    pub blocks: Vec<ScheduledBlockRecord>,
}

pub struct NewDailyPlan<'a> {
    pub id: &'a str,
    pub plan_date: &'a str,
    pub version: i64,
    pub planned_work_minutes: u32,
    pub reserve_minutes: u32,
    pub feasibility: &'a str,
    pub created_at: i64,
}

pub struct PlannerRepo;

impl PlannerRepo {
    pub fn planning_tasks(conn: &Connection) -> Result<Vec<PlanningTaskRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT t.id, t.title, t.description, t.priority,
                    t.estimated_minutes, t.requires_deep_work, t.deadline,
                    EXISTS(
                        SELECT 1
                        FROM task_dependencies d
                        JOIN tasks predecessor ON predecessor.id = d.depends_on_task_id
                        WHERE d.task_id = t.id
                          AND (
                            (d.dependency_type = 'FINISH_TO_START'
                                AND predecessor.status != 'COMPLETED')
                            OR
                            (d.dependency_type = 'START_TO_START'
                                AND predecessor.status NOT IN ('IN_PROGRESS','PAUSED','COMPLETED'))
                          )
                    ) AS dependency_blocked,
                    t.context_tag, t.energy_level, t.postponement_count, t.created_at
             FROM tasks t
             WHERE t.archived = 0 AND t.deleted_at IS NULL
               AND t.status IN ('TODO','SCHEDULED','PAUSED')
             ORDER BY t.created_at, t.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(PlanningTaskRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                priority: row.get(3)?,
                estimated_minutes: row.get(4)?,
                requires_deep_work: row.get(5)?,
                deadline: row.get(6)?,
                dependency_blocked: row.get(7)?,
                context_tag: row.get(8)?,
                energy_level: row.get(9)?,
                postponement_count: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn active_plan(
        conn: &Connection,
        plan_date: &str,
    ) -> Result<Option<DailyPlanRecord>, DbError> {
        let plan = conn
            .query_row(
                "SELECT id, plan_date, version, planned_work_minutes,
                        workload_reserve_minutes, feasibility, created_at
                 FROM daily_plans
                 WHERE plan_date = ?1 AND status = 'ACTIVE'
                 ORDER BY version DESC LIMIT 1",
                [plan_date],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, u32>(3)?,
                        row.get::<_, u32>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            id,
            plan_date,
            version,
            planned_work_minutes,
            workload_reserve_minutes,
            feasibility,
            created_at,
        )) = plan
        else {
            return Ok(None);
        };

        let mut statement = conn.prepare(
            "SELECT q.task_id, t.title, t.description, t.priority,
                    q.estimated_minutes, q.score, t.deadline
             FROM flexible_queue_items q
             JOIN tasks t ON t.id = q.task_id
             WHERE q.daily_plan_id = ?1 AND q.status IN ('QUEUED','SELECTED','STARTED')
             ORDER BY q.position",
        )?;
        let items = statement
            .query_map([&id], |row| {
                Ok(QueueItemRecord {
                    task_id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    priority: row.get(3)?,
                    estimated_minutes: row.get(4)?,
                    score: row.get(5)?,
                    deadline: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut statement = conn.prepare(
            "SELECT b.id, b.task_id, t.title, t.description, b.scheduled_start,
                    b.scheduled_end, b.duration_minutes, b.priority, b.status
             FROM scheduled_blocks b
             JOIN tasks t ON t.id = b.task_id
             JOIN daily_plans p ON p.id = b.daily_plan_id
             WHERE (
                   (b.daily_plan_id = ?1 AND b.status IN ('PLANNED','ACTIVE'))
                   OR (
                       p.plan_date = ?2 AND b.status = 'ACTIVE'
                       AND EXISTS (
                           SELECT 1 FROM task_sessions s
                           WHERE s.scheduled_block_id = b.id AND s.end_time IS NULL
                       )
                       AND NOT EXISTS (
                           SELECT 1
                           FROM scheduled_blocks current_block
                           WHERE current_block.daily_plan_id = ?1
                             AND current_block.task_id = b.task_id
                             AND current_block.status IN ('PLANNED','ACTIVE')
                       )
                   )
             )
             ORDER BY b.scheduled_start",
        )?;
        let blocks = statement
            .query_map(rusqlite::params![&id, &plan_date], |row| {
                Ok(ScheduledBlockRecord {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    scheduled_start: row.get(4)?,
                    scheduled_end: row.get(5)?,
                    duration_minutes: row.get(6)?,
                    priority: row.get(7)?,
                    status: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(DailyPlanRecord {
            id,
            plan_date,
            version,
            planned_work_minutes,
            workload_reserve_minutes,
            feasibility,
            created_at,
            items,
            blocks,
        }))
    }

    pub fn active_session_blocks(
        conn: &Connection,
        plan_date: &str,
    ) -> Result<Vec<ScheduledBlockRecord>, DbError> {
        let mut statement = conn.prepare(
            "SELECT b.id, b.task_id, t.title, t.description, b.scheduled_start,
                    b.scheduled_end, b.duration_minutes, b.priority, b.status
             FROM scheduled_blocks b
             JOIN daily_plans p ON p.id = b.daily_plan_id
             JOIN tasks t ON t.id = b.task_id
             JOIN task_sessions s ON s.scheduled_block_id = b.id
             WHERE p.plan_date = ?1 AND s.end_time IS NULL AND b.status = 'ACTIVE'
             ORDER BY b.scheduled_start",
        )?;
        let rows = statement.query_map([plan_date], |row| {
            Ok(ScheduledBlockRecord {
                id: row.get(0)?,
                task_id: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                scheduled_start: row.get(4)?,
                scheduled_end: row.get(5)?,
                duration_minutes: row.get(6)?,
                priority: row.get(7)?,
                status: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn next_version(conn: &Connection, plan_date: &str) -> Result<i64, DbError> {
        Ok(conn.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM daily_plans WHERE plan_date = ?1",
            [plan_date],
            |row| row.get(0),
        )?)
    }

    pub fn commit_plan(
        conn: &Connection,
        plan: &NewDailyPlan<'_>,
    ) -> Result<Option<String>, DbError> {
        let previous_id: Option<String> = conn
            .query_row(
                "SELECT id FROM daily_plans
                 WHERE plan_date = ?1 AND status = 'ACTIVE'
                 ORDER BY version DESC LIMIT 1",
                [plan.plan_date],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(previous_id) = &previous_id {
            conn.execute(
                "UPDATE daily_plans SET status = 'SUPERSEDED', updated_at = ?2 WHERE id = ?1",
                rusqlite::params![previous_id, plan.created_at],
            )?;
        }
        conn.execute(
            "INSERT INTO daily_plans
                (id, plan_date, status, version, supersedes_plan_id,
                 planning_mode, planned_work_minutes, workload_reserve_minutes,
                 feasibility, created_at, updated_at)
             VALUES (?1, ?2, 'ACTIVE', ?3, ?4, 'AUTO', ?5, ?6, ?7, ?8, ?8)",
            rusqlite::params![
                plan.id,
                plan.plan_date,
                plan.version,
                previous_id,
                plan.planned_work_minutes,
                plan.reserve_minutes,
                plan.feasibility,
                plan.created_at,
            ],
        )?;
        Ok(previous_id)
    }

    pub fn insert_queue_item(
        conn: &Connection,
        plan_id: &str,
        position: usize,
        item: &PlanningTaskRecord,
        score: f64,
        urgency: u16,
        queued_at: i64,
    ) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO flexible_queue_items
                (id, daily_plan_id, task_id, position, score, urgency, context_tag,
                 energy_level, estimated_minutes, status, queued_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'QUEUED', ?10)",
            rusqlite::params![
                focus_domain::ids::EntityId::new().to_string(),
                plan_id,
                item.id,
                position as i64,
                score,
                urgency,
                item.context_tag,
                item.energy_level,
                item.estimated_minutes,
                queued_at,
            ],
        )?;
        Ok(())
    }

    pub fn insert_scheduled_block(
        conn: &Connection,
        plan_id: &str,
        item: &PlanningTaskRecord,
        block_id: &str,
        scheduled_start: i64,
        scheduled_end: i64,
        created_at: i64,
    ) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO scheduled_blocks
                (id, daily_plan_id, task_id, project_id, goal_id, block_type,
                 scheduled_start, scheduled_end, duration_minutes, status, priority,
                 rationale, source, created_at, updated_at)
             SELECT ?1, ?2, t.id, t.project_id, t.goal_id, 'DEEP_WORK',
                    ?3, ?4, ?5, 'PLANNED', t.priority,
                    'Scheduled by the deterministic daily planner', 'PLANNER', ?6, ?6
             FROM tasks t
             WHERE t.id = ?7 AND t.archived = 0 AND t.deleted_at IS NULL",
            rusqlite::params![
                block_id,
                plan_id,
                scheduled_start,
                scheduled_end,
                item.estimated_minutes,
                created_at,
                item.id,
            ],
        )?;
        Ok(())
    }

    pub fn copy_active_block(
        conn: &Connection,
        source_block_id: &str,
        plan_id: &str,
        copied_block_id: &str,
        created_at: i64,
    ) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO scheduled_blocks
                (id, daily_plan_id, task_id, project_id, goal_id, block_type,
                 scheduled_start, scheduled_end, actual_start, actual_end,
                 duration_minutes, status, priority, rationale, source, adaptive,
                 created_at, updated_at)
             SELECT ?1, ?2, task_id, project_id, goal_id, block_type,
                    scheduled_start, scheduled_end, actual_start, actual_end,
                    duration_minutes, 'ACTIVE', priority, rationale, source, adaptive,
                    ?3, ?3
             FROM scheduled_blocks
             WHERE id = ?4 AND status = 'ACTIVE'",
            rusqlite::params![copied_block_id, plan_id, created_at, source_block_id],
        )?;
        Ok(())
    }

    pub fn insert_plan_event(
        conn: &Connection,
        event_id: &str,
        plan_id: &str,
        occurred_at: i64,
        previous_id: Option<&str>,
        after_payload: &str,
        undo_payload: &str,
    ) -> Result<(), DbError> {
        conn.execute(
            "INSERT INTO schedule_events
                (id, daily_plan_id, occurred_at, event_type, actor, subject_id,
                 before_payload, after_payload, undo_payload, created_at)
             VALUES (?1, ?2, ?3, 'PLAN_REPLANNED', 'SCHEDULER', ?2, ?4, ?5, ?6, ?3)",
            rusqlite::params![
                event_id,
                plan_id,
                occurred_at,
                previous_id.map(|id| serde_json::json!({ "activePlanId": id }).to_string()),
                after_payload,
                undo_payload,
            ],
        )?;
        Ok(())
    }
}
