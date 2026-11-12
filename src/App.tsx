import { useEffect } from 'react'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'
import { AppLayout } from './app/layout'
import { AnalyticsScreen } from './features/analytics/AnalyticsScreen'
import { GoalsScreen } from './features/goals/GoalsScreen'
import { InboxScreen } from './features/inbox/InboxScreen'
import { ProjectsScreen } from './features/projects/ProjectsScreen'
import { SettingsScreen } from './features/settings/SettingsScreen'
import { TodayScreen } from './features/today/TodayScreen'
import { useAppStore } from './stores/appStore'
import { useSettingsStore } from './stores/settingsStore'

export function App() {
  const loadApp = useAppStore((s) => s.load)
  const loadSettings = useSettingsStore((s) => s.load)

  useEffect(() => {
    void loadApp()
    void loadSettings()
  }, [loadApp, loadSettings])

  return (
    <HashRouter>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<Navigate to="/today" replace />} />
          <Route path="/today" element={<TodayScreen />} />
          <Route path="/inbox" element={<InboxScreen />} />
          <Route path="/projects" element={<ProjectsScreen />} />
          <Route path="/goals" element={<GoalsScreen />} />
          <Route path="/analytics" element={<AnalyticsScreen />} />
          <Route path="/settings" element={<SettingsScreen />} />
          <Route path="*" element={<Navigate to="/today" replace />} />
        </Route>
      </Routes>
    </HashRouter>
  )
}