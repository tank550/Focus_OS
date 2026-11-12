import {
  CalendarCheck2,
  Compass,
  Crosshair,
  FolderKanban,
  Inbox,
  Settings2,
  Target,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { NavLink } from 'react-router-dom'

interface NavItem {
  to: string
  label: string
  icon: LucideIcon
  badge?: number
}

const NAV: NavItem[] = [
  { to: '/today', label: 'Today', icon: Compass },
  { to: '/inbox', label: 'Inbox', icon: Inbox, badge: 0 },
  { to: '/projects', label: 'Projects', icon: FolderKanban },
  { to: '/goals', label: 'Goals', icon: Target },
  { to: '/analytics', label: 'Analytics', icon: CalendarCheck2 },
  { to: '/settings', label: 'Settings', icon: Settings2 },
]

export function Sidebar() {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-surface/60">
      <div className="flex items-center gap-2 px-5 pb-2 pt-6">
        <Crosshair className="h-5 w-5 text-accent" />
        <span className="text-sm font-semibold tracking-widest text-text">
          FOCUS&nbsp;OS
        </span>
      </div>

      <nav className="mt-4 flex flex-1 flex-col gap-1 px-3">
        {NAV.map(({ to, label, icon: Icon, badge }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors duration-150 ${
                isActive
                  ? 'bg-surface-2 text-text'
                  : 'text-text-muted hover:bg-surface-2/60 hover:text-text'
              }`
            }
          >
            <Icon className="h-4 w-4" />
            <span className="flex-1">{label}</span>
            {badge !== undefined && badge > 0 && (
              <span className="rounded-full bg-accent/20 px-2 py-0.5 text-[10px] font-medium text-accent">
                {badge}
              </span>
            )}
          </NavLink>
        ))}
      </nav>

      <div className="px-5 pb-4 text-[11px] leading-relaxed text-text-muted">
        Hybrid scheduling · adaptive autonomy
      </div>
    </aside>
  )
}