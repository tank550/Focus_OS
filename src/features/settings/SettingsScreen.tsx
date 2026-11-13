import { useEffect, useState } from 'react'
import { Card } from '../../components/ui/Card'
import { bridge } from '../../lib/tauri'
import { useAppStore } from '../../stores/appStore'
import { useSettingsStore } from '../../stores/settingsStore'
import type {
  AiSettings,
  GithubRepository,
  GithubSourceSummary,
  PortfolioReport,
  ScheduleSettings,
} from '../../shared/contracts'

const inputClass =
  'w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent'

export function SettingsScreen() {
  const { appInfo, health, inTauri } = useAppStore()
  const {
    settings,
    error: settingsError,
    saveAi,
    saveSchedule,
    storeAiApiKey,
    removeAiApiKey,
  } = useSettingsStore()
  const [aiConfig, setAiConfig] = useState<AiSettings>(settings.ai)
  const [apiKey, setApiKey] = useState('')
  const [savingAi, setSavingAi] = useState(false)
  const [scheduleConfig, setScheduleConfig] = useState<ScheduleSettings>(
    settings.schedule,
  )
  const [savingSchedule, setSavingSchedule] = useState(false)
  const [scheduleMessage, setScheduleMessage] = useState<string | null>(null)
  const [aiMessage, setAiMessage] = useState<string | null>(null)
  const [githubToken, setGithubToken] = useState('')
  const [githubConnected, setGithubConnected] = useState(false)
  const [githubRepositories, setGithubRepositories] = useState<
    GithubRepository[]
  >([])
  const [githubSourceSummary, setGithubSourceSummary] =
    useState<GithubSourceSummary | null>(null)
  const [githubBusy, setGithubBusy] = useState(false)
  const [githubMessage, setGithubMessage] = useState<string | null>(null)
  const [githubError, setGithubError] = useState<string | null>(null)
  const [portfolioReports, setPortfolioReports] = useState<PortfolioReport[]>([])
  const [selectedPortfolioId, setSelectedPortfolioId] = useState<string | null>(
    null,
  )
  const [portfolioDraft, setPortfolioDraft] = useState('')
  const [portfolioBusy, setPortfolioBusy] = useState(false)
  const [portfolioMessage, setPortfolioMessage] = useState<string | null>(null)
  const [portfolioError, setPortfolioError] = useState<string | null>(null)

  const selectedPortfolio = portfolioReports.find(
    (report) => report.id === selectedPortfolioId,
  )

  useEffect(() => {
    setAiConfig(settings.ai)
  }, [settings.ai])

  useEffect(() => {
    setScheduleConfig(settings.schedule)
  }, [settings.schedule])

  useEffect(() => {
    if (!inTauri) return
    let active = true
    void bridge
      .githubTokenStatus()
      .then((connected) => {
        if (active) setGithubConnected(connected)
      })
      .catch((reason) => {
        if (active)
          setGithubError(
            reason instanceof Error ? reason.message : String(reason),
          )
      })
    return () => {
      active = false
    }
  }, [inTauri])

  useEffect(() => {
    if (!inTauri) return
    let active = true
    void bridge
      .portfolioAssessmentsList()
      .then((reports) => {
        if (!active) return
        setPortfolioReports(reports)
        if (reports[0]) {
          setSelectedPortfolioId(reports[0].id)
          setPortfolioDraft(reports[0].draftMarkdown)
        }
      })
      .catch((reason) => {
        if (active)
          setPortfolioError(
            reason instanceof Error ? reason.message : String(reason),
          )
      })
    return () => {
      active = false
    }
  }, [inTauri])

  const saveConfiguration = async () => {
    setSavingAi(true)
    setAiMessage(null)
    try {
      await saveAi({
        ...aiConfig,
        baseUrl: aiConfig.baseUrl?.trim() || null,
        model: aiConfig.model?.trim() || null,
      })
      setAiMessage('AI configuration saved.')
    } catch (reason) {
      setAiMessage(String(reason))
    } finally {
      setSavingAi(false)
    }
  }

  const saveApiKey = async () => {
    setSavingAi(true)
    setAiMessage(null)
    try {
      await storeAiApiKey(apiKey)
      setApiKey('')
      setAiMessage('API key stored in the operating system credential store.')
    } catch (reason) {
      setAiMessage(String(reason))
    } finally {
      setSavingAi(false)
    }
  }

  const clearApiKey = async () => {
    setSavingAi(true)
    setAiMessage(null)
    try {
      await removeAiApiKey()
      setApiKey('')
      setAiMessage('Stored API key removed.')
    } catch (reason) {
      setAiMessage(String(reason))
    } finally {
      setSavingAi(false)
    }
  }

  const saveScheduleConfiguration = async () => {
    setSavingSchedule(true)
    setScheduleMessage(null)
    try {
      await saveSchedule(scheduleConfig)
      setScheduleMessage('Schedule settings saved.')
    } catch (reason) {
      setScheduleMessage(
        reason instanceof Error ? reason.message : String(reason),
      )
    } finally {
      setSavingSchedule(false)
    }
  }

  const connectGithub = async () => {
    setGithubBusy(true)
    setGithubError(null)
    setGithubMessage(null)
    try {
      const repositories = await bridge.githubTokenStore(githubToken)
      setGithubToken('')
      setGithubConnected(true)
      setGithubRepositories(repositories)
      setGithubMessage(`Connected. Found ${repositories.length} repositories.`)
    } catch (reason) {
      setGithubError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setGithubBusy(false)
    }
  }

  const refreshGithubRepositories = async () => {
    setGithubBusy(true)
    setGithubError(null)
    setGithubMessage(null)
    try {
      const repositories = await bridge.githubRepositories()
      setGithubRepositories(repositories)
      setGithubMessage(`Found ${repositories.length} repositories.`)
    } catch (reason) {
      setGithubError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setGithubBusy(false)
    }
  }

  const inspectGithubRepository = async (fullName: string) => {
    setGithubBusy(true)
    setGithubError(null)
    setGithubMessage(null)
    setGithubSourceSummary(null)
    try {
      const summary = await bridge.githubInspectRepository(fullName)
      setGithubSourceSummary(summary)
      setGithubMessage(
        'Repository inspected locally. Source contents were not sent to NVIDIA.',
      )
    } catch (reason) {
      setGithubError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setGithubBusy(false)
    }
  }

  const analyzePortfolioRepository = async () => {
    if (!githubSourceSummary) return
    setPortfolioBusy(true)
    setPortfolioError(null)
    setPortfolioMessage(null)
    try {
      const report = await bridge.portfolioAnalyzeRepository(
        githubSourceSummary.fullName,
        githubSourceSummary,
      )
      setPortfolioReports((reports) => [
        report,
        ...reports.filter((item) => item.id !== report.id),
      ])
      setSelectedPortfolioId(report.id)
      setPortfolioDraft(report.draftMarkdown)
      setPortfolioMessage('Portfolio assessment saved on this device.')
    } catch (reason) {
      setPortfolioError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setPortfolioBusy(false)
    }
  }

  const selectPortfolioReport = (report: PortfolioReport) => {
    setSelectedPortfolioId(report.id)
    setPortfolioDraft(report.draftMarkdown)
    setPortfolioMessage(null)
    setPortfolioError(null)
  }

  const savePortfolioDraft = async () => {
    if (!selectedPortfolio) return
    setPortfolioBusy(true)
    setPortfolioError(null)
    setPortfolioMessage(null)
    try {
      await bridge.portfolioSaveDraft(selectedPortfolio.id, portfolioDraft)
      const updatedAt = Date.now()
      setPortfolioReports((reports) =>
        reports.map((report) =>
          report.id === selectedPortfolio.id
            ? { ...report, draftMarkdown: portfolioDraft.trim(), updatedAt }
            : report,
        ),
      )
      setPortfolioDraft(portfolioDraft.trim())
      setPortfolioMessage('Portfolio draft saved.')
    } catch (reason) {
      setPortfolioError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setPortfolioBusy(false)
    }
  }

  const disconnectGithub = async () => {
    setGithubBusy(true)
    setGithubError(null)
    setGithubMessage(null)
    try {
      await bridge.githubTokenRemove()
      setGithubConnected(false)
      setGithubRepositories([])
      setGithubSourceSummary(null)
      setGithubToken('')
      setGithubMessage('GitHub token removed from the OS credential store.')
    } catch (reason) {
      setGithubError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setGithubBusy(false)
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <header>
        <p className="text-xs uppercase tracking-wider text-text-muted">
          Settings
        </p>
        <h1 className="mt-1 text-2xl font-semibold">Settings</h1>
      </header>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <Card title="AI provider">
          <div className="flex flex-col gap-4">
            <label className="flex flex-col gap-1.5 text-xs text-text-muted">
              Provider
              <select
                className={inputClass}
                value={aiConfig.provider}
                onChange={(event) =>
                  setAiConfig({
                    ...aiConfig,
                    provider: event.target.value as AiSettings['provider'],
                  })
                }
                disabled={!inTauri || savingAi}
              >
                <option value="nim">NVIDIA NIM (real AI)</option>
                <option value="mock">Mock (fixed demo response; no AI)</option>
              </select>
            </label>

            {aiConfig.provider === 'nim' ? (
              <>
                <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                  NIM API base URL
                  <input
                    className={inputClass}
                    value={aiConfig.baseUrl ?? ''}
                    placeholder="https://integrate.api.nvidia.com"
                    onChange={(event) =>
                      setAiConfig({ ...aiConfig, baseUrl: event.target.value })
                    }
                    disabled={!inTauri || savingAi}
                  />
                </label>
                <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                  Model ID from your NVIDIA catalog
                  <input
                    className={inputClass}
                    value={aiConfig.model ?? ''}
                    placeholder="e.g. model ID shown in your NIM catalog"
                    onChange={(event) =>
                      setAiConfig({ ...aiConfig, model: event.target.value })
                    }
                    disabled={!inTauri || savingAi}
                  />
                </label>
                <div className="rounded-md border border-border bg-surface-2 p-3">
                  <p className="text-xs text-text-muted">
                    API key:{' '}
                    {settings.ai.apiKeyStored
                      ? 'stored in the OS credential store'
                      : 'not configured'}
                  </p>
                  <label className="mt-3 flex flex-col gap-1.5 text-xs text-text-muted">
                    NVIDIA API key
                    <input
                      className={inputClass}
                      type="password"
                      autoComplete="new-password"
                      value={apiKey}
                      onChange={(event) => setApiKey(event.target.value)}
                      disabled={!inTauri || savingAi}
                    />
                  </label>
                  <div className="mt-3 flex flex-wrap gap-2">
                    <button
                      type="button"
                      className="rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text disabled:opacity-40"
                      onClick={() => void saveApiKey()}
                      disabled={!inTauri || savingAi || !apiKey.trim()}
                    >
                      Store API key securely
                    </button>
                    {settings.ai.apiKeyStored && (
                      <button
                        type="button"
                        className="rounded-md border border-danger/30 px-3 py-1.5 text-xs text-danger hover:bg-danger/10 disabled:opacity-40"
                        onClick={() => void clearApiKey()}
                        disabled={!inTauri || savingAi}
                      >
                        Remove API key
                      </button>
                    )}
                  </div>
                </div>
                <p className="text-xs leading-relaxed text-text-muted">
                  A note is sent to NVIDIA only when you explicitly choose to
                  analyze it. The API key is never written to SQLite.
                </p>
              </>
            ) : (
              <p className="text-xs leading-relaxed text-text-muted">
                Mock is a fixed test response, not a model. It sends no note
                content anywhere and will not create tasks or projects.
              </p>
            )}

            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                onClick={() => void saveConfiguration()}
                disabled={!inTauri || savingAi}
              >
                {savingAi ? 'Saving…' : 'Save AI settings'}
              </button>
              {aiMessage && (
                <p className="text-xs text-text-muted" role="status">
                  {aiMessage}
                </p>
              )}
              {settingsError && (
                <p className="text-xs text-danger" role="alert">
                  {settingsError}
                </p>
              )}
            </div>
          </div>
        </Card>

        <Card title="GitHub (read-only)">
          <div className="flex flex-col gap-4">
            <p className="text-sm text-text-muted">
              Connect a fine-grained personal access token to list repositories
              you own, including private repositories. Focus OS cannot modify
              GitHub content.
            </p>
            <p className="text-xs leading-relaxed text-text-muted">
              Grant Metadata: Read-only and Contents: Read-only for all
              repositories. The token is stored in the OS credential store.
              Inspection reads up to 12 small source/test files locally to
              derive counts; raw source is neither stored nor sent to NVIDIA.
              Portfolio evaluation sends only the exact derived summary shown
              below to your configured NIM endpoint, after you confirm. Source
              contents are never sent.
            </p>
            <p className="text-xs text-text-muted" role="status">
              {githubConnected ? 'Token stored securely.' : 'Not connected.'}
            </p>
            {!githubConnected && (
              <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                Fine-grained GitHub token
                <input
                  className={inputClass}
                  type="password"
                  autoComplete="new-password"
                  value={githubToken}
                  onChange={(event) => setGithubToken(event.target.value)}
                  disabled={!inTauri || githubBusy}
                />
              </label>
            )}
            <div className="flex flex-wrap gap-2">
              {!githubConnected ? (
                <button
                  type="button"
                  className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                  onClick={() => void connectGithub()}
                  disabled={!inTauri || githubBusy || !githubToken.trim()}
                >
                  {githubBusy ? 'Connecting…' : 'Connect and list repositories'}
                </button>
              ) : (
                <>
                  <button
                    type="button"
                    className="rounded-md bg-accent px-4 py-2 text-sm font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                    onClick={() => void refreshGithubRepositories()}
                    disabled={githubBusy}
                  >
                    {githubBusy ? 'Loading…' : 'Refresh repositories'}
                  </button>
                  <button
                    type="button"
                    className="rounded-md border border-danger/30 px-3 py-1.5 text-xs text-danger hover:bg-danger/10 disabled:opacity-40"
                    onClick={() => void disconnectGithub()}
                    disabled={githubBusy}
                  >
                    Disconnect
                  </button>
                </>
              )}
            </div>
            {githubMessage && (
              <p className="text-xs text-text-muted" role="status">
                {githubMessage}
              </p>
            )}
            {githubError && (
              <p className="text-xs text-danger" role="alert">
                {githubError}
              </p>
            )}
            {githubRepositories.length > 0 && (
              <ul className="max-h-72 overflow-y-auto rounded-md border border-border">
                {githubRepositories.map((repository) => (
                  <li
                    key={repository.id}
                    className="border-b border-border px-3 py-3 last:border-b-0"
                  >
                    <p className="text-sm font-medium">{repository.fullName}</p>
                    <p className="mt-1 text-xs text-text-muted">
                      {repository.private ? 'Private' : 'Public'}
                      {repository.language ? ` · ${repository.language}` : ''}
                      {` · ${repository.stargazersCount} stars`}
                    </p>
                    {repository.description && (
                      <p className="mt-1 text-xs text-text-muted">
                        {repository.description}
                      </p>
                    )}
                    <button
                      type="button"
                      className="mt-2 rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text disabled:opacity-40"
                      onClick={() =>
                        void inspectGithubRepository(repository.fullName)
                      }
                      disabled={githubBusy}
                    >
                      {githubBusy ? 'Inspecting…' : 'Inspect source locally'}
                    </button>
                    {githubSourceSummary?.fullName === repository.fullName && (
                      <div className="mt-3 rounded-md border border-border bg-surface-2 p-3">
                        <p className="text-xs font-medium">
                          Local source profile · {githubSourceSummary.defaultBranch}
                        </p>
                        <p className="mt-1 text-xs text-text-muted">
                          {githubSourceSummary.sourceFileCount} source files ·{' '}
                          {githubSourceSummary.testFileCount} test files ·{' '}
                          {githubSourceSummary.documentationFileCount} docs ·{' '}
                          {githubSourceSummary.inspectedFileCount} inspected ·{' '}
                          {githubSourceSummary.inspectedLines} nonblank lines ·{' '}
                          {githubSourceSummary.declarationsFound} declarations
                        </p>
                        <p className="mt-1 text-xs text-text-muted">
                          Languages:{' '}
                          {Object.entries(githubSourceSummary.languages)
                            .map(([language, bytes]) => `${language} (${bytes} B)`)
                            .join(', ') || 'not detected'}
                        </p>
                        <p className="mt-1 text-xs text-text-muted">
                          README: {githubSourceSummary.hasReadme ? 'yes' : 'no'}
                          {githubSourceSummary.manifestFiles.length > 0 &&
                            ` · manifests: ${githubSourceSummary.manifestFiles.join(', ')}`}
                        </p>
                        <div className="mt-3 rounded-md border border-border p-3">
                          <p className="text-xs font-medium">
                            Confirm before AI evaluation
                          </p>
                          <p className="mt-1 text-xs text-text-muted">
                            Review the exact derived summary that will be sent
                            to your configured NIM endpoint. It contains no
                            source code or file contents.
                          </p>
                          <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap break-words text-[11px] text-text-muted">
                            {JSON.stringify(githubSourceSummary, null, 2)}
                          </pre>
                          <button
                            type="button"
                            className="mt-3 rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-white hover:bg-accent/90 disabled:opacity-40"
                            onClick={() => void analyzePortfolioRepository()}
                            disabled={
                              !inTauri ||
                              portfolioBusy ||
                              !settings.ai.enabled ||
                              settings.ai.provider !== 'nim' ||
                              !settings.ai.model?.trim() ||
                              !settings.ai.apiKeyStored
                            }
                          >
                            {portfolioBusy
                              ? 'Analyzing…'
                              : 'Confirm and send summary to NIM'}
                          </button>
                        </div>
                      </div>
                    )}
                  </li>
                ))}
              </ul>
            )}
            {portfolioError && (
              <p className="text-xs text-danger" role="alert">
                {portfolioError}
              </p>
            )}
            {portfolioMessage && (
              <p className="text-xs text-text-muted" role="status">
                {portfolioMessage}
              </p>
            )}
            {portfolioReports.length > 0 && (
              <section className="flex flex-col gap-3 border-t border-border pt-4">
                <h3 className="text-sm font-medium">Portfolio assessments</h3>
                <div className="flex flex-wrap gap-2">
                  {portfolioReports.map((report) => (
                    <button
                      key={report.id}
                      type="button"
                      className={`rounded-md border px-3 py-1.5 text-xs ${
                        report.id === selectedPortfolioId
                          ? 'border-accent text-accent'
                          : 'border-border text-text-muted hover:text-text'
                      }`}
                      onClick={() => selectPortfolioReport(report)}
                    >
                      {report.repoFullName} · {report.evaluation.readinessScore}/100
                    </button>
                  ))}
                </div>
                {selectedPortfolio && (
                  <div className="flex flex-col gap-3 rounded-md border border-border bg-surface-2 p-3">
                    <div>
                      <p className="text-sm font-medium">
                        {selectedPortfolio.repoFullName} ·{' '}
                        {selectedPortfolio.evaluation.readinessScore}/100
                      </p>
                      <p className="mt-1 text-xs leading-relaxed text-text-muted">
                        {selectedPortfolio.evaluation.projectSummary}
                      </p>
                    </div>
                    <div className="grid gap-3 sm:grid-cols-2">
                      <div>
                        <p className="text-xs font-medium">Strengths</p>
                        <ul className="mt-1 list-disc pl-4 text-xs text-text-muted">
                          {selectedPortfolio.evaluation.strengths.map((item) => (
                            <li key={item}>{item}</li>
                          ))}
                        </ul>
                      </div>
                      <div>
                        <p className="text-xs font-medium">Gaps to address</p>
                        <ul className="mt-1 list-disc pl-4 text-xs text-text-muted">
                          {selectedPortfolio.evaluation.gaps.length > 0 ? (
                            selectedPortfolio.evaluation.gaps.map((item) => (
                              <li key={item}>{item}</li>
                            ))
                          ) : (
                            <li>No gaps were identified.</li>
                          )}
                        </ul>
                      </div>
                    </div>
                    <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                      Editable CV / portfolio draft ·{' '}
                      {portfolioDraft.length}/12,000 characters
                      <textarea
                        className={`${inputClass} min-h-56 resize-y font-mono`}
                        value={portfolioDraft}
                        maxLength={12000}
                        onChange={(event) => setPortfolioDraft(event.target.value)}
                        disabled={portfolioBusy}
                      />
                    </label>
                    <div className="flex flex-wrap items-center gap-3">
                      <button
                        type="button"
                        className="rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text disabled:opacity-40"
                        onClick={() => void savePortfolioDraft()}
                        disabled={
                          !inTauri ||
                          portfolioBusy ||
                          !portfolioDraft.trim() ||
                          portfolioDraft === selectedPortfolio.draftMarkdown
                        }
                      >
                        {portfolioBusy ? 'Saving…' : 'Save draft'}
                      </button>
                      <p className="text-[11px] text-text-muted">
                        AI confidence: {Math.round(selectedPortfolio.evaluation.confidence * 100)}%.
                        Verify every claim before using it.
                      </p>
                    </div>
                  </div>
                )}
              </section>
            )}
          </div>
        </Card>

        <Card title="Schedule">
          <div className="flex flex-col gap-4">
            <p className="text-xs leading-relaxed text-text-muted">
              These limits are used by the local daily planner. At least 30
              minutes or the configured reserve (whichever is larger) stays
              unplanned.
            </p>
            <div className="grid grid-cols-2 gap-3">
              <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                Working day starts
                <input
                  className={inputClass}
                  type="time"
                  value={scheduleConfig.workingHoursStart}
                  onChange={(event) =>
                    setScheduleConfig({
                      ...scheduleConfig,
                      workingHoursStart: event.target.value,
                    })
                  }
                  disabled={!inTauri || savingSchedule}
                />
              </label>
              <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                Working day ends
                <input
                  className={inputClass}
                  type="time"
                  value={scheduleConfig.workingHoursEnd}
                  onChange={(event) =>
                    setScheduleConfig({
                      ...scheduleConfig,
                      workingHoursEnd: event.target.value,
                    })
                  }
                  disabled={!inTauri || savingSchedule}
                />
              </label>
              <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                Daily work limit (minutes)
                <input
                  className={inputClass}
                  type="number"
                  min={1}
                  max={1440}
                  step={1}
                  value={scheduleConfig.maxDailyMinutes}
                  onChange={(event) =>
                    setScheduleConfig({
                      ...scheduleConfig,
                      maxDailyMinutes: Number(event.target.value),
                    })
                  }
                  disabled={!inTauri || savingSchedule}
                />
              </label>
              <label className="flex flex-col gap-1.5 text-xs text-text-muted">
                Protected reserve (%)
                <input
                  className={inputClass}
                  type="number"
                  min={0}
                  max={100}
                  step={1}
                  value={Math.round(scheduleConfig.reserveRatio * 100)}
                  onChange={(event) =>
                    setScheduleConfig({
                      ...scheduleConfig,
                      reserveRatio: Number(event.target.value) / 100,
                    })
                  }
                  disabled={!inTauri || savingSchedule}
                />
              </label>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                className="rounded-md border border-border px-3 py-1.5 text-xs text-text-muted hover:text-text disabled:opacity-40"
                onClick={() => void saveScheduleConfiguration()}
                disabled={
                  !inTauri ||
                  savingSchedule ||
                  !/^([01]\d|2[0-3]):[0-5]\d$/.test(
                    scheduleConfig.workingHoursStart,
                  ) ||
                  !/^([01]\d|2[0-3]):[0-5]\d$/.test(
                    scheduleConfig.workingHoursEnd,
                  ) ||
                  !Number.isInteger(scheduleConfig.maxDailyMinutes) ||
                  scheduleConfig.maxDailyMinutes < 1 ||
                  scheduleConfig.maxDailyMinutes > 1440 ||
                  !Number.isFinite(scheduleConfig.reserveRatio) ||
                  scheduleConfig.reserveRatio < 0 ||
                  scheduleConfig.reserveRatio > 1 ||
                  scheduleConfig.workingHoursEnd <= scheduleConfig.workingHoursStart
                }
              >
                {savingSchedule ? 'Saving…' : 'Save schedule'}
              </button>
              {scheduleMessage && (
                <p className="text-xs text-text-muted" role="status">
                  {scheduleMessage}
                </p>
              )}
            </div>
          </div>
        </Card>

        <Card title="Application">
          {inTauri && appInfo ? (
            <p className="text-sm">
              {appInfo.name} v{appInfo.version} · Tauri {appInfo.tauriVersion}
            </p>
          ) : (
            <p className="text-sm text-text-muted">
              Running in browser preview mode.
            </p>
          )}
          {health && (
            <p className="mt-2 text-xs text-text-muted">
              Database:{' '}
              <span className={health.ok ? 'text-success' : 'text-danger'}>
                {health.ok ? health.db : health.error}
              </span>{' '}
              · {health.migrationsApplied} migration(s) applied
              {health.databasePath && (
                <span className="block break-all">{health.databasePath}</span>
              )}
            </p>
          )}
        </Card>

        <Card title="Appearance">
          <p className="text-sm">
            Theme:{' '}
            <span className="font-mono text-text">{settings.appearance.theme}</span>
          </p>
        </Card>
      </div>
    </div>
  )
}