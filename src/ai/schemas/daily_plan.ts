import { z } from 'zod';

/** DailyPlanProposal — main objective, top-3 priorities, workload hints. */
export const DailyPlanProposal = z.object({
  main_objective: z.string(),
  top_priorities: z.array(z.object({
    task_id: z.string().uuid(),
    title: z.string(),
    estimated_minutes: z.number().int().min(1),
  })),
  planned_work_minutes: z.number().int().min(0),
  feasibility_hint: z.string().optional(),
});