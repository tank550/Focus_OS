import { z } from 'zod';

/** GoalPlan — decompose a quarterly goal → milestones, themes, risks. */
export const GoalPlan = z.object({
  title: z.string(),
  milestones: z.array(z.object({
    title: z.string(),
    description: z.string().optional(),
    target_date: z.string().optional(), // 'YYYY-MM-DD'
    success_criteria: z.array(z.string()),
  })),
  themes: z.array(z.string()),
  risks: z.array(z.string()),
});