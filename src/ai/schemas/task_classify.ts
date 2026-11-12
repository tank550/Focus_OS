import { z } from 'zod';

/** TaskClassification — classify a task into the domain vocabulary. */
export const TaskClassification = z.object({
  kind: z.enum(['TASK', 'IDEA']),
  project_id: z.string().uuid().optional(),
  goal_id: z.string().uuid().optional(),
  estimated_minutes: z.number().int().min(1),
  priority: z.number().int().min(0).max(100),
  next_step: z.string().optional(),
});