import { z } from 'zod';

/** BrainDumpAnalysis — classify raw inbox text. */
export const BrainDumpAnalysis = z.object({
  kind: z.enum(['TASK', 'IDEA', 'PROJECT', 'RESPONSIBILITY', 'OBLIGATION', 'RESEARCH', 'EXPERIMENT', 'AMBITION', 'NOTE']),
  project_guess: z.string().optional(),
  goal_guess: z.string().optional(),
  estimated_duration_minutes: z.number().int().min(1),
  priority: z.number().int().min(0).max(100),
  deadline: z.string().optional(), // 'YYYY-MM-DD' or null
  actionability: z.enum(['ACTIONABLE', 'MAYBE', 'NOT_ACTIONABLE']),
  next_step: z.string().optional(),
  dependencies: z.array(z.string()).optional(),
  confidence: z.number().min(0).max(1),
});