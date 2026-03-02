import { useEffect } from "react";
import { useAIStore } from "@/stores/useAIStore";
import type { AIProviderType } from "@/types/ai";

export function Settings() {
  const { config, loadConfig } = useAIStore();

  useEffect(() => {
    loadConfig();
  }, []);

  return (
    <div className="mx-auto max-w-2xl space-y-8">
      <div>
        <h1 className="text-2xl font-bold text-foreground">Settings</h1>
        <p className="text-sm text-muted-foreground">
          Configure your LearnForge experience
        </p>
      </div>

      {/* AI Provider Settings */}
      <section className="space-y-4">
        <h2 className="text-lg font-semibold text-foreground">AI Provider</h2>
        <div className="rounded-lg border border-border bg-card p-6 space-y-4">
          <div>
            <label className="block text-sm font-medium text-foreground mb-1.5">
              Provider
            </label>
            <select className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm">
              <option value="claude">Claude (Anthropic)</option>
              <option value="openai">OpenAI</option>
              <option value="ollama">Ollama (Local)</option>
              <option value="custom">Custom Endpoint</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-foreground mb-1.5">
              API Key
            </label>
            <input
              type="password"
              placeholder="sk-ant-..."
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
            <p className="mt-1 text-xs text-muted-foreground">
              Your API key is stored locally and never sent to our servers
            </p>
          </div>
          <div>
            <label className="block text-sm font-medium text-foreground mb-1.5">
              Model
            </label>
            <input
              type="text"
              placeholder="claude-sonnet-4-20250514"
              defaultValue={config?.model ?? ""}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
        </div>
      </section>

      {/* Learning Preferences */}
      <section className="space-y-4">
        <h2 className="text-lg font-semibold text-foreground">Learning Preferences</h2>
        <div className="rounded-lg border border-border bg-card p-6 space-y-4">
          <div>
            <label className="block text-sm font-medium text-foreground mb-1.5">
              Daily Goal (minutes)
            </label>
            <input
              type="number"
              defaultValue={30}
              min={5}
              max={240}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-foreground mb-1.5">
              Preferred Session Duration (minutes)
            </label>
            <input
              type="number"
              defaultValue={25}
              min={5}
              max={120}
              className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            />
          </div>
        </div>
      </section>
    </div>
  );
}
