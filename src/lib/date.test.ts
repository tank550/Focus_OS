import { describe, expect, it } from 'vitest'
import { clockLabel, localDateKey, minutesToLabel } from './date'

describe('date utilities', () => {
  it('formats local date keys as YYYY-MM-DD', () => {
    expect(localDateKey(new Date(2026, 10, 5))).toBe('2026-11-05')
    expect(localDateKey(new Date(2026, 0, 3))).toBe('2026-01-03')
  })

  it('formats 24h clock labels with zero padding', () => {
    expect(clockLabel(9, 5)).toBe('09:05')
    expect(clockLabel(23, 59)).toBe('23:59')
  })

  it('formats durations compactly', () => {
    expect(minutesToLabel(25)).toBe('25m')
    expect(minutesToLabel(90)).toBe('1h 30m')
    expect(minutesToLabel(120)).toBe('2h')
    expect(minutesToLabel(0)).toBe('0m')
  })

  it('clamps negative durations', () => {
    expect(minutesToLabel(-5)).toBe('0m')
  })
})