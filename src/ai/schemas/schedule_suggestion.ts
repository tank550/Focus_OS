import { z } from 'zod';

/** ScheduleSuggestion — alternative placements / rationale for a hypothetical move. */
export const ScheduleSuggestion = z.object({
  rationale: z.string(),
  would_displace: z.string().uuid(),
  new_start: z.number().int(), // epoch ms
  new_end: z.number().int(),
  score: z.number().min(0).max(1),
});