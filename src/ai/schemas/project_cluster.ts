import { z } from 'zod';

/** ProjectClustering — group inbox entries into projects; detect relationships. */
export const ProjectClustering = z.object({
  entries: z.array(z.object({
    inbox_item_id: z.string().uuid(),
    suggested_project_id: z.string().uuid().optional(),
    confidence: z.number().min(0).max(1),
  })),
  newly_detected_project: z.object({
    title: z.string(),
    description: z.string().optional(),
  }).optional(),
});