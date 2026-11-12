import { z } from 'zod';

/** DailyReviewAnalysis — accomplishments, blockers, planning accuracy. */
export const DailyReviewAnalysis = z.object({
  accomplishments: z.array(z.string()),
  blockers: z.array(z.string()),
  planning_accuracy: z.number().min(0).max(1),
  insight: z.string().optional(),
});