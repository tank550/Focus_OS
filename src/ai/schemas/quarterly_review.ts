import { z } from 'zod';

/** QuarterlyReviewAnalysis — outcomes, lessons, next-quarter recommendations. */
export const QuarterlyReviewAnalysis = z.object({
  outcomes: z.object({
    goals_completed: z.number().int().min(0),
    goals_abandoned: z.number().int().min(0),
  }),
  lessons: z.array(z.string()),
  recommendations: z.array(z.object({
    title: z.string(),
    description: z.string(),
  })),
});