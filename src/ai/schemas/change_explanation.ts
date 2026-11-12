import { z } from 'zod';

/** ChangeExplanation — "why did this move?" pieces rendered inline in the UI. */
export const ChangeExplanation = z.object({
  primary_reason: z.string(),
  secondary_factors: z.array(z.string()),
  autonomy_level: z.enum(['AUTO_EXECUTE', 'PROPOSE', 'REQUIRE_CONFIRMATION']),
});