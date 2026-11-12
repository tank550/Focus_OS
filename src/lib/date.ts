/** Local calendar date key, same shape as `daily_plans.plan_date`: YYYY-MM-DD. */
export function localDateKey(d: Date = new Date()): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

/** 'HH:MM' (24h) from hour/minute, e.g. `clockLabel(9, 5)` => '09:05'. */
export function clockLabel(h: number, m: number): string {
  return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
}

/** Compact duration label: minutesToLabel(90) => '1h 30m', minutesToLabel(25) => '25m'. */
export function minutesToLabel(min: number): string {
  const total = Math.max(0, Math.round(min))
  if (total < 60) return `${total}m`
  const h = Math.floor(total / 60)
  const rem = total % 60
  return rem === 0 ? `${h}h` : `${h}h ${rem}m`
}