import { useEffect, useState } from "react";
import { useTheme } from "@/hooks/useTheme";
import * as commands from "@/lib/tauri-commands";
import type { ProviderAuthStatus, LoginRequest } from "@/types/ai";
import {
  Shield,
  Key,
  ChevronDown,
  ChevronUp,
  CheckCircle2,
  Wifi,
  WifiOff,
  Sun,
  Moon,
  AlertTriangle,
  Server,
  Zap,
  ExternalLink,
  RefreshCw,
  Loader2,
} from "lucide-react";

// ── Provider Metadata ──

interface ProviderMeta {
  id: string;
  name: string;
  company: string;
  description: string;
  keyPlaceholder: string;
  defaultModel: string;
}

const PROVIDERS: ProviderMeta[] = [
  {
    id: "claude",
    name: "Claude",
    company: "Anthropic",
    description: "Advanced reasoning and analysis. Recommended for learning paths.",
    keyPlaceholder: "sk-ant-api03-...",
    defaultModel: "claude-sonnet-4-20250514",
  },
  {
    id: "openai",
    name: "ChatGPT",
    company: "OpenAI",
    description: "Versatile language model with broad knowledge coverage.",
    keyPlaceholder: "sk-...",
    defaultModel: "gpt-4o",
  },
  {
    id: "gemini",
    name: "Gemini",
    company: "Google",
    description: "Multimodal AI with strong technical understanding.",
    keyPlaceholder: "AIza...",
    defaultModel: "gemini-2.0-flash",
  },
];

// ── Settings Page ──

export function Settings() {
  const { theme, toggleTheme } = useTheme();

  const [authStatuses, setAuthStatuses] = useState<ProviderAuthStatus[]>([]);
  const [expandedBYOK, setExpandedBYOK] = useState<string | null>(null);
  const [byokInputs, setByokInputs] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  // Ollama state
  const [ollamaHost, setOllamaHost] = useState("http://localhost:11434");
  const [ollamaModel, setOllamaModel] = useState("llama3");
  const [ollamaStatus, setOllamaStatus] = useState<
    "idle" | "checking" | "connected" | "error"
  >("idle");
  const [oauthPending, setOauthPending] = useState<string | null>(null);
  const [setupTokenExpanded, setSetupTokenExpanded] = useState(false);
  const [setupTokenInput, setSetupTokenInput] = useState("");

  async function loadAuthStatus() {
    try {
      const statuses = await commands.getAuthStatus();
      setAuthStatuses(statuses);
    } catch (err) {
      console.error("Failed to load auth status:", err);
    }
  }

  useEffect(() => {
    loadAuthStatus();
  }, []);

  // ── Derived State ──

  function getProviderStatus(providerId: string): ProviderAuthStatus | undefined {
    return authStatuses.find((s) => s.provider === providerId);
  }

  function getActiveProviderId(): string | undefined {
    return authStatuses.find((s) => s.isActive)?.provider;
  }

  // ── Handlers ──

  async function handleBYOKSave(providerId: string) {
    const key = byokInputs[providerId];
    if (!key?.trim()) return;

    setActionError(null);
    setLoading(providerId);
    try {
      const provider = PROVIDERS.find((p) => p.id === providerId);
      const request: LoginRequest = {
        provider: providerId,
        method: "api-key",
        credential: key.trim(),
        model: provider?.defaultModel,
      };
      await commands.loginProvider(request);
      await loadAuthStatus();
      setExpandedBYOK(null);
      setByokInputs((prev) => ({ ...prev, [providerId]: "" }));
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setActionError(msg);
    } finally {
      setLoading(null);
    }
  }

  async function handleDisconnect(providerId: string) {
    setLoading(providerId);
    try {
      await commands.logoutProvider(providerId);
      await loadAuthStatus();
    } catch (err) {
      console.error("Failed to disconnect:", err);
    } finally {
      setLoading(null);
    }
  }

  async function handleSetActive(providerId: string) {
    if (!providerId) return;
    setActionError(null);
    setLoading(providerId);
    try {
      await commands.setActiveProvider(providerId);
      await loadAuthStatus();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setActionError(`Cannot set ${providerId} as active: ${msg}. Connect it first.`);
    } finally {
      setLoading(null);
    }
  }

  async function handleOllamaSave() {
    setLoading("ollama");
    try {
      const request: LoginRequest = {
        provider: "ollama",
        method: "ollama",
        model: ollamaModel,
        baseUrl: ollamaHost,
      };
      await commands.loginProvider(request);
      await loadAuthStatus();
    } catch (err) {
      console.error("Failed to configure Ollama:", err);
    } finally {
      setLoading(null);
    }
  }

  function handleOllamaCheck() {
    setOllamaStatus("checking");
    // TODO: Invoke a Tauri command to test the Ollama connection
    setTimeout(() => {
      setOllamaStatus("connected");
    }, 1200);
  }

  async function handleSaveSetupToken() {
    if (!setupTokenInput.trim()) return;
    setActionError(null);
    setLoading("claude");
    try {
      await commands.saveSetupToken(setupTokenInput.trim());
      await loadAuthStatus();
      setSetupTokenExpanded(false);
      setSetupTokenInput("");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setActionError(msg);
    } finally {
      setLoading(null);
    }
  }

  async function handleOAuthLogin(providerId: string) {
    setOauthPending(providerId);
    try {
      await commands.startOAuthLogin(providerId);

      // Poll for completion
      const maxAttempts = 30; // 60 seconds at 2s intervals
      for (let i = 0; i < maxAttempts; i++) {
        await new Promise((r) => setTimeout(r, 2000));
        const status = await commands.checkOAuthStatus(providerId);
        if (status.completed) {
          await loadAuthStatus();
          setOauthPending(null);
          return;
        }
      }
      // Timeout
      setOauthPending(null);
    } catch (err) {
      console.error("OAuth login failed:", err);
      setOauthPending(null);
    }
  }

  // ── Render Helpers ──

  const activeProviderId = getActiveProviderId();

  function getActiveProviderLabel(): string {
    if (activeProviderId === "ollama") return "Ollama (Local)";
    const p = PROVIDERS.find((pr) => pr.id === activeProviderId);
    return p ? `${p.name} (${p.company})` : activeProviderId ?? "None";
  }

  function isProviderConnected(id: string): boolean {
    if (id === "ollama") {
      const status = getProviderStatus("ollama");
      return status?.authenticated ?? false;
    }
    return getProviderStatus(id)?.authenticated ?? false;
  }

  return (
    <div className="mx-auto max-w-3xl space-y-8 pb-12">
      {/* Page Header */}
      <div>
        <h1 className="text-2xl font-bold text-foreground">Settings</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Configure AI providers, preferences, and application behavior.
        </p>
      </div>

      {/* Active Provider Indicator */}
      <div className="glass rounded-xl p-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10">
              <Zap size={18} className="text-primary" />
            </div>
            <div>
              <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                Active Provider
              </p>
              <p className="text-sm font-semibold text-foreground">
                {getActiveProviderLabel()}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-2">
            {activeProviderId && isProviderConnected(activeProviderId) ? (
              <>
                <span className="inline-block h-2.5 w-2.5 rounded-full bg-emerald-500" />
                <span className="text-xs font-medium text-emerald-500">
                  Connected
                </span>
              </>
            ) : (
              <>
                <span className="inline-block h-2.5 w-2.5 rounded-full bg-zinc-400" />
                <span className="text-xs font-medium text-muted-foreground">
                  Not connected
                </span>
              </>
            )}
          </div>
        </div>
      </div>

      {/* ── AI Providers ── */}
      <section className="space-y-4">
        <h2 className="text-lg font-semibold text-foreground">AI Providers</h2>

        <div className="space-y-3">
          {PROVIDERS.map((provider) => {
            const status = getProviderStatus(provider.id);
            const isConnected = status?.authenticated ?? false;
            const isBYOKExpanded = expandedBYOK === provider.id;
            const isActive = activeProviderId === provider.id;
            const isLoading = loading === provider.id;

            return (
              <div
                key={provider.id}
                className={`glass rounded-xl p-5 transition-all ${
                  isActive ? "ring-1 ring-primary/40" : ""
                }`}
              >
                {/* Provider Header */}
                <div className="flex items-start justify-between">
                  <div className="flex items-start gap-3">
                    <div className="mt-0.5 flex h-10 w-10 items-center justify-center rounded-lg bg-secondary">
                      <Shield size={20} className="text-foreground" />
                    </div>
                    <div>
                      <div className="flex items-center gap-2">
                        <h3 className="text-sm font-semibold text-foreground">
                          {provider.name}
                        </h3>
                        <span className="text-xs text-muted-foreground">
                          {provider.company}
                        </span>
                      </div>
                      <p className="mt-0.5 text-xs text-muted-foreground">
                        {provider.description}
                      </p>
                    </div>
                  </div>

                  {/* Status Indicator */}
                  <div className="flex items-center gap-2">
                    {isConnected ? (
                      <>
                        <span className="inline-block h-2.5 w-2.5 rounded-full bg-emerald-500" />
                        <span className="text-xs font-medium text-emerald-500">
                          Connected
                        </span>
                      </>
                    ) : (
                      <>
                        <span className="inline-block h-2.5 w-2.5 rounded-full bg-zinc-400" />
                        <span className="text-xs font-medium text-muted-foreground">
                          Disconnected
                        </span>
                      </>
                    )}
                  </div>
                </div>

                {/* Claude ToS Warning */}
                {provider.id === "claude" && (
                  <div className="mt-3 flex items-start gap-2 rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2.5">
                    <AlertTriangle
                      size={14}
                      className="mt-0.5 shrink-0 text-amber-500"
                    />
                    <p className="text-xs leading-relaxed text-amber-200/80">
                      Anthropic updated their Terms of Service in February 2026.
                      By connecting, you agree to their revised data usage and
                      privacy policies.{" "}
                      <a
                        href="#"
                        className="inline-flex items-center gap-0.5 underline underline-offset-2 hover:text-amber-300"
                      >
                        Review changes
                        <ExternalLink size={10} />
                      </a>
                    </p>
                  </div>
                )}

                {/* Actions */}
                <div className="mt-4 flex flex-wrap items-center gap-2">
                  {isConnected ? (
                    <>
                      {!isActive && (
                        <button
                          onClick={() => handleSetActive(provider.id)}
                          disabled={isLoading}
                          className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
                        >
                          Set as Active
                        </button>
                      )}
                      {isActive && (
                        <span className="flex items-center gap-1.5 rounded-lg bg-primary/10 px-3 py-2 text-xs font-medium text-primary">
                          <CheckCircle2 size={14} />
                          Active Provider
                        </span>
                      )}
                      <button
                        onClick={() => handleDisconnect(provider.id)}
                        disabled={isLoading}
                        className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:border-destructive/50 hover:text-destructive disabled:opacity-50"
                      >
                        Disconnect
                      </button>
                    </>
                  ) : (
                    <>
                      {/* OAuth for OpenAI / Gemini */}
                      {(provider.id === "openai" || provider.id === "gemini") && (
                        <button
                          onClick={() => handleOAuthLogin(provider.id)}
                          disabled={oauthPending === provider.id}
                          className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
                        >
                          {oauthPending === provider.id ? (
                            <>
                              <Loader2 size={12} className="animate-spin" />
                              Waiting for browser...
                            </>
                          ) : (
                            <>
                              <ExternalLink size={12} />
                              Sign in with {provider.name}
                            </>
                          )}
                        </button>
                      )}

                      {/* Claude setup-token flow */}
                      {provider.id === "claude" && (
                        <button
                          onClick={() => setSetupTokenExpanded(!setupTokenExpanded)}
                          className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                        >
                          <Shield size={12} />
                          Use Setup Token
                          {setupTokenExpanded ? (
                            <ChevronUp size={12} />
                          ) : (
                            <ChevronDown size={12} />
                          )}
                        </button>
                      )}

                      {/* BYOK Toggle */}
                      <button
                        onClick={() =>
                          setExpandedBYOK(
                            isBYOKExpanded ? null : provider.id,
                          )
                        }
                        className="flex items-center gap-1.5 rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
                      >
                        <Key size={12} />
                        Use API Key
                        {isBYOKExpanded ? (
                          <ChevronUp size={12} />
                        ) : (
                          <ChevronDown size={12} />
                        )}
                      </button>
                    </>
                  )}
                </div>

                {/* Claude Setup Token Section */}
                {provider.id === "claude" && setupTokenExpanded && !isConnected && (
                  <div className="mt-3 space-y-3 rounded-lg border border-border/50 bg-secondary/30 p-4">
                    <div>
                      <label className="mb-1.5 block text-xs font-medium text-foreground">
                        Setup Token
                      </label>
                      <p className="mb-2 text-[11px] leading-relaxed text-muted-foreground">
                        Run{" "}
                        <code className="rounded bg-secondary px-1.5 py-0.5 text-[11px] font-medium text-foreground">
                          claude setup-token
                        </code>{" "}
                        in your terminal, then paste the token below. This uses your existing Claude subscription.
                      </p>
                      <input
                        type="password"
                        placeholder="sk-ant-oat01-..."
                        value={setupTokenInput}
                        onChange={(e) => setSetupTokenInput(e.target.value)}
                        className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                      />
                      <p className="mt-1 text-[11px] text-muted-foreground">
                        Your token is stored locally and used via Anthropic's official OAuth API.
                      </p>
                    </div>
                    <button
                      onClick={handleSaveSetupToken}
                      disabled={!setupTokenInput.trim() || isLoading}
                      className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      {loading === "claude" && (
                        <Loader2 size={12} className="animate-spin" />
                      )}
                      {loading === "claude" ? "Validating..." : "Save and Connect"}
                    </button>
                    {actionError && (
                      <p className="text-xs text-destructive">{actionError}</p>
                    )}
                  </div>
                )}

                {/* BYOK Expanded Section */}
                {isBYOKExpanded && !isConnected && (
                  <div className="mt-3 space-y-3 rounded-lg border border-border/50 bg-secondary/30 p-4">
                    <div>
                      <label className="mb-1.5 block text-xs font-medium text-foreground">
                        API Key
                      </label>
                      <input
                        type="password"
                        placeholder={provider.keyPlaceholder}
                        value={byokInputs[provider.id] ?? ""}
                        onChange={(e) =>
                          setByokInputs((prev) => ({
                            ...prev,
                            [provider.id]: e.target.value,
                          }))
                        }
                        className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                      />
                      <p className="mt-1 text-[11px] text-muted-foreground">
                        Your key is stored locally and never sent to external
                        servers.
                      </p>
                    </div>
                    <button
                      onClick={() => handleBYOKSave(provider.id)}
                      disabled={!byokInputs[provider.id]?.trim() || isLoading}
                      className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      Save and Connect
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </section>

      {/* ── Ollama (Local AI) ── */}
      <section className="space-y-4">
        <h2 className="text-lg font-semibold text-foreground">
          Ollama -- Local AI
        </h2>

        <div className="glass rounded-xl p-5 space-y-4">
          <div className="flex items-start gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-secondary">
              <Server size={20} className="text-foreground" />
            </div>
            <div>
              <h3 className="text-sm font-semibold text-foreground">
                Local Inference
              </h3>
              <p className="mt-0.5 text-xs text-muted-foreground">
                Run models locally with Ollama. No API key needed -- your data
                stays on your machine.
              </p>
            </div>
          </div>

          {/* Host URL */}
          <div>
            <label className="mb-1.5 block text-xs font-medium text-foreground">
              Host URL
            </label>
            <div className="flex gap-2">
              <input
                type="text"
                value={ollamaHost}
                onChange={(e) => setOllamaHost(e.target.value)}
                placeholder="http://localhost:11434"
                className="flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              />
              <button
                onClick={handleOllamaCheck}
                disabled={ollamaStatus === "checking"}
                className="flex items-center gap-1.5 rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground disabled:opacity-50"
              >
                {ollamaStatus === "checking" ? (
                  <RefreshCw size={14} className="animate-spin" />
                ) : ollamaStatus === "connected" ? (
                  <Wifi size={14} className="text-emerald-500" />
                ) : ollamaStatus === "error" ? (
                  <WifiOff size={14} className="text-destructive" />
                ) : (
                  <Wifi size={14} />
                )}
                {ollamaStatus === "checking"
                  ? "Checking..."
                  : ollamaStatus === "connected"
                    ? "Connected"
                    : ollamaStatus === "error"
                      ? "Failed"
                      : "Test Connection"}
              </button>
            </div>
          </div>

          {/* Model */}
          <div>
            <label className="mb-1.5 block text-xs font-medium text-foreground">
              Model
            </label>
            <input
              type="text"
              value={ollamaModel}
              onChange={(e) => setOllamaModel(e.target.value)}
              placeholder="llama3"
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            />
            <p className="mt-1 text-[11px] text-muted-foreground">
              Enter the model name as shown by{" "}
              <code className="rounded bg-secondary px-1 py-0.5 text-[11px]">
                ollama list
              </code>
            </p>
          </div>

          {/* Save / Set Active */}
          <div className="flex items-center gap-2">
            <button
              onClick={handleOllamaSave}
              disabled={loading === "ollama"}
              className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
            >
              {activeProviderId === "ollama"
                ? "Update Configuration"
                : "Set as Active Provider"}
            </button>
            {activeProviderId === "ollama" && (
              <span className="flex items-center gap-1.5 text-xs font-medium text-primary">
                <CheckCircle2 size={14} />
                Active
              </span>
            )}
          </div>
        </div>
      </section>

      {/* ── Preferences ── */}
      <section className="space-y-4">
        <h2 className="text-lg font-semibold text-foreground">Preferences</h2>

        <div className="glass rounded-xl p-5 space-y-5">
          {/* Theme Toggle */}
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium text-foreground">Theme</p>
              <p className="text-xs text-muted-foreground">
                Switch between dark and light mode
              </p>
            </div>
            <button
              onClick={toggleTheme}
              className="flex items-center gap-2 rounded-lg border border-border px-4 py-2 text-xs font-medium text-foreground transition-colors hover:bg-secondary"
            >
              {theme === "dark" ? (
                <>
                  <Moon size={14} />
                  Dark
                </>
              ) : (
                <>
                  <Sun size={14} />
                  Light
                </>
              )}
            </button>
          </div>

          {/* Divider */}
          <div className="border-t border-border" />

          {/* Default Provider */}
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm font-medium text-foreground">
                Default Provider
              </p>
              <p className="text-xs text-muted-foreground">
                Provider used for new learning tracks
              </p>
            </div>
            <select
              value={activeProviderId ?? ""}
              onChange={(e) => handleSetActive(e.target.value)}
              className="rounded-lg border border-input bg-background px-3 py-2 text-xs font-medium text-foreground focus:outline-none focus:ring-1 focus:ring-ring disabled:opacity-50"
            >
              <option value="" disabled>-- select a connected provider --</option>
              {[
                { value: "claude", label: "Claude (Anthropic)" },
                { value: "openai", label: "ChatGPT (OpenAI)" },
                { value: "gemini", label: "Gemini (Google)" },
                { value: "ollama", label: "Ollama (Local)" },
              ].map((opt) => (
                <option
                  key={opt.value}
                  value={opt.value}
                  disabled={!isProviderConnected(opt.value)}
                >
                  {opt.label}{!isProviderConnected(opt.value) ? " (not connected)" : ""}
                </option>
              ))}
            </select>
          </div>
          {actionError && (
            <p className="mt-1 text-xs text-destructive">{actionError}</p>
          )}
        </div>
      </section>
    </div>
  );
}
