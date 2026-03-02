import { useEffect, useState } from "react";
import { useTheme } from "@/hooks/useTheme";
import { useAIStore } from "@/stores/useAIStore";
import type { AIProviderType } from "@/types/ai";
import {
  Shield,
  Key,
  ChevronDown,
  ChevronUp,
  CheckCircle2,
  Circle,
  Wifi,
  WifiOff,
  Sun,
  Moon,
  AlertTriangle,
  Server,
  Zap,
  ExternalLink,
  RefreshCw,
} from "lucide-react";

// ── Provider Metadata ──

interface ProviderMeta {
  id: AIProviderType;
  name: string;
  company: string;
  description: string;
  oauthLabel: string;
  keyPlaceholder: string;
  defaultModel: string;
}

const PROVIDERS: ProviderMeta[] = [
  {
    id: "claude",
    name: "Claude",
    company: "Anthropic",
    description: "Advanced reasoning and analysis. Recommended for learning paths.",
    oauthLabel: "Sign in with Anthropic",
    keyPlaceholder: "sk-ant-api03-...",
    defaultModel: "claude-sonnet-4-20250514",
  },
  {
    id: "openai",
    name: "ChatGPT",
    company: "OpenAI",
    description: "Versatile language model with broad knowledge coverage.",
    oauthLabel: "Sign in with OpenAI",
    keyPlaceholder: "sk-...",
    defaultModel: "gpt-4o",
  },
  {
    id: "custom",
    name: "Gemini",
    company: "Google",
    description: "Multimodal AI with strong technical understanding.",
    oauthLabel: "Sign in with Google",
    keyPlaceholder: "AIza...",
    defaultModel: "gemini-2.0-flash",
  },
];

// ── Connection State ──

interface ProviderConnection {
  connected: boolean;
  method: "oauth" | "apikey" | null;
  apiKey?: string;
}

type ConnectionMap = Record<string, ProviderConnection>;

// ── Settings Page ──

export function Settings() {
  const { config, loadConfig, updateConfig } = useAIStore();
  const { theme, toggleTheme } = useTheme();

  const [connections, setConnections] = useState<ConnectionMap>(() => {
    const map: ConnectionMap = {};
    for (const p of PROVIDERS) {
      map[p.id] = { connected: false, method: null };
    }
    return map;
  });

  const [expandedBYOK, setExpandedBYOK] = useState<string | null>(null);
  const [byokInputs, setByokInputs] = useState<Record<string, string>>({});
  const [activeProvider, setActiveProvider] = useState<AIProviderType>(
    config?.type ?? "claude",
  );

  // Ollama state
  const [ollamaHost, setOllamaHost] = useState("http://localhost:11434");
  const [ollamaModel, setOllamaModel] = useState("llama3");
  const [ollamaStatus, setOllamaStatus] = useState<
    "idle" | "checking" | "connected" | "error"
  >("idle");

  useEffect(() => {
    loadConfig();
  }, []);

  useEffect(() => {
    if (config) {
      setActiveProvider(config.type);
      if (config.type === "ollama") {
        setOllamaHost(config.baseUrl ?? "http://localhost:11434");
        setOllamaModel(config.model);
      }
      // If config has an apiKey set for a known provider, mark it connected
      if (config.apiKey) {
        setConnections((prev) => ({
          ...prev,
          [config.type]: {
            connected: true,
            method: "apikey",
            apiKey: config.apiKey,
          },
        }));
      }
    }
  }, [config]);

  // ── Handlers ──

  function handleOAuthConnect(providerId: string) {
    // In a real implementation this would open the OAuth flow via Tauri deep links.
    // For now we simulate a successful connection.
    setConnections((prev) => ({
      ...prev,
      [providerId]: { connected: true, method: "oauth" },
    }));
  }

  function handleBYOKSave(providerId: string) {
    const key = byokInputs[providerId];
    if (!key?.trim()) return;

    setConnections((prev) => ({
      ...prev,
      [providerId]: { connected: true, method: "apikey", apiKey: key.trim() },
    }));
    setExpandedBYOK(null);

    // If this is the active provider, push config update
    if (providerId === activeProvider) {
      const provider = PROVIDERS.find((p) => p.id === providerId);
      updateConfig({
        type: providerId as AIProviderType,
        apiKey: key.trim(),
        model: config?.model ?? provider?.defaultModel ?? "",
        maxTokens: config?.maxTokens ?? 4096,
        temperature: config?.temperature ?? 0.7,
      });
    }
  }

  function handleDisconnect(providerId: string) {
    setConnections((prev) => ({
      ...prev,
      [providerId]: { connected: false, method: null },
    }));
    setByokInputs((prev) => ({ ...prev, [providerId]: "" }));
  }

  function handleSetActiveProvider(type: AIProviderType) {
    setActiveProvider(type);
    const conn = connections[type];
    const provider = PROVIDERS.find((p) => p.id === type);
    updateConfig({
      type,
      apiKey: conn?.apiKey ?? config?.apiKey ?? "",
      model: provider?.defaultModel ?? config?.model ?? "",
      maxTokens: config?.maxTokens ?? 4096,
      temperature: config?.temperature ?? 0.7,
    });
  }

  function handleOllamaCheck() {
    setOllamaStatus("checking");
    // Simulate connection check -- in production this would invoke a Tauri command
    setTimeout(() => {
      setOllamaStatus("connected");
    }, 1200);
  }

  function handleOllamaSave() {
    setActiveProvider("ollama");
    updateConfig({
      type: "ollama",
      model: ollamaModel,
      baseUrl: ollamaHost,
      maxTokens: config?.maxTokens ?? 4096,
      temperature: config?.temperature ?? 0.7,
    });
  }

  // ── Render Helpers ──

  function getActiveProviderLabel(): string {
    if (activeProvider === "ollama") return "Ollama (Local)";
    const p = PROVIDERS.find((pr) => pr.id === activeProvider);
    return p ? `${p.name} (${p.company})` : activeProvider;
  }

  function isProviderConnected(id: string): boolean {
    if (id === "ollama") return ollamaStatus === "connected";
    return connections[id]?.connected ?? false;
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
            {isProviderConnected(activeProvider) ? (
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
            const conn = connections[provider.id];
            const isConnected = conn?.connected ?? false;
            const isBYOKExpanded = expandedBYOK === provider.id;
            const isActive = activeProvider === provider.id;

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
                          onClick={() =>
                            handleSetActiveProvider(
                              provider.id as AIProviderType,
                            )
                          }
                          className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
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
                        className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground transition-colors hover:border-destructive/50 hover:text-destructive"
                      >
                        Disconnect
                      </button>
                    </>
                  ) : (
                    <>
                      {/* OAuth Button */}
                      <button
                        onClick={() => handleOAuthConnect(provider.id)}
                        className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                      >
                        {provider.oauthLabel}
                      </button>

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
                      disabled={!byokInputs[provider.id]?.trim()}
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
              className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
              {activeProvider === "ollama"
                ? "Update Configuration"
                : "Set as Active Provider"}
            </button>
            {activeProvider === "ollama" && (
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
              value={activeProvider}
              onChange={(e) =>
                handleSetActiveProvider(e.target.value as AIProviderType)
              }
              className="rounded-lg border border-input bg-background px-3 py-2 text-xs font-medium text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            >
              <option value="claude">Claude (Anthropic)</option>
              <option value="openai">ChatGPT (OpenAI)</option>
              <option value="custom">Gemini (Google)</option>
              <option value="ollama">Ollama (Local)</option>
            </select>
          </div>
        </div>
      </section>
    </div>
  );
}
