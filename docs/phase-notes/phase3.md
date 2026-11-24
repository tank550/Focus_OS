# Phase 3 — Scheduling engine (status notes)

Goal: deterministic local hybrid planning — score, partition, allocate deep
work into blocks, keep the rest in a flexible queue, and report infeasibility
honestly.

## Exit criteria / checklist

- [x] Deterministic scoring with deadline urgency and tie-breaks
- [x] Partition: deep work (explicit or ≥ 60 min) vs flexible queue
- [x] Clock-time blocks from configured working hours and timezone offset
- [x] Capacity limit + protected workload reserve
- [x] Explicit overflow report instead of an overloaded day
- [x] Dependency-blocked tasks are excluded and surfaced
- [x] One transaction per plan commit (plan + blocks + queue + audit event)
- [x] Version bump on replan and preservation of the active session block
- [ ] Break and buffer cadence (`minBreakMinutes`, `minBufferMinutes`)
- [ ] Deep-work windows, energy fit, context-switch scoring
- [ ] EDD pre-pass for hard deadlines (currently score-based)
- [ ] Replan protocol: freeze horizon + diff + scenario engines A/B/E/F
- [ ] Split rules for `is_splittable` tasks
- [ ] Gap-fit "start next" selection for the flexible queue

## Delivered in this phase

- `planner::plan_day` builds a local plan from open tasks.
- Deep-work tasks receive `scheduled_blocks`; the rest become ranked
  `flexible_queue_items`; overflow and blocked lists are explicit.
- `Today` renders blocks, the flexible queue, overflow, and blocked tasks, and
  can start/pause/complete a focus session.
- Replanning keeps a running block even when the rest of the day is recomputed.

## Known gaps

- The planner treats the working day as one continuous span; meetings, breaks,
  and buffers are not modelled yet.
- Adaptive replanning and plan diffs are not implemented.
- Queue selection is a ranked list; it does not yet answer "what fits the next
  gap?".
