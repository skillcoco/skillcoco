import { useState, useEffect } from "react";
import * as commands from "@/lib/tauri-commands";
import { Key, Loader2, CheckCircle2 } from "lucide-react";

/**
 * Phase 11 — YouTube Data API v3 key field (D-05/D-06).
 *
 * Saves the key via the existing `ai` credential store (provider "youtube"),
 * which is the same path the backend reads from to gate video discovery.
 * No key → RelatedVideosPanel renders nothing (D-06 — panel silent on empty list).
 *
 * Security: input type="password" masks the key on screen (T-11-09 mitigate).
 * Key is never echoed to console.log. Stored only via the credential store.
 *
 * UAT fix (Phase 11): reflects whether a key is configured on mount via
 * `is_youtube_key_configured` (fail-soft: treats IPC errors as not-configured).
 * Shows a "Key configured" indicator when true, "Key removed." confirmation
 * after removal, and "Key saved." after a successful save.
 */

interface SettingsYouTubeSectionProps {
  /** Optional callback fired after a save or remove so the parent can reload auth status */
  onKeySaved?: () => void;
  onKeyRemoved?: () => void;
}

export function SettingsYouTubeSection({
  onKeySaved,
  onKeyRemoved,
}: SettingsYouTubeSectionProps) {
  const [ytKeyInput, setYtKeyInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [removed, setRemoved] = useState(false);
  // `configured` reflects whether a key is currently stored in the credential
  // store. Null = still loading (mount check in progress).
  const [configured, setConfigured] = useState<boolean | null>(null);

  // On mount: check whether a YouTube key is already configured.
  // Fail-soft: any IPC error is treated as not-configured (false).
  useEffect(() => {
    let cancelled = false;
    commands
      .isYoutubeKeyConfigured()
      .then((result) => {
        if (!cancelled) setConfigured(result);
      })
      .catch(() => {
        if (!cancelled) setConfigured(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSaveYouTubeKey() {
    const key = ytKeyInput.trim();
    if (!key) return;

    setError(null);
    setSaved(false);
    setRemoved(false);
    setLoading(true);
    try {
      await commands.loginProvider({
        provider: "youtube",
        method: "api-key",
        credential: key,
      });
      setYtKeyInput("");
      setSaved(true);
      setConfigured(true);
      onKeySaved?.();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
    } finally {
      setLoading(false);
    }
  }

  async function handleRemoveYouTubeKey() {
    setError(null);
    setSaved(false);
    setRemoved(false);
    setLoading(true);
    try {
      await commands.logoutProvider("youtube");
      setConfigured(false);
      setYtKeyInput("");
      setRemoved(true);
      onKeyRemoved?.();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">YouTube Data API v3</h2>

      <div className="glass rounded-xl p-5 space-y-4">
        <div className="flex items-start gap-3">
          <div className="mt-0.5 flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-secondary">
            <Key size={20} className="text-foreground" />
          </div>
          <div>
            <p className="text-sm font-medium text-foreground">YouTube Data API v3 key</p>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Powers the &quot;Related videos&quot; panel at the bottom of each lesson.
              Without a key the panel is hidden — lessons still work normally (D-06).
              Get a key at{" "}
              <a
                href="https://console.cloud.google.com/apis/library/youtube.googleapis.com"
                target="_blank"
                rel="noreferrer"
                className="underline underline-offset-2 hover:text-foreground"
              >
                Google Cloud Console
              </a>
              .
            </p>
          </div>
        </div>

        {/* Configured indicator — shown when a key is stored */}
        {configured && (
          <p
            data-testid="yt-key-configured-indicator"
            className="flex items-center gap-1.5 text-xs text-emerald-500"
          >
            <CheckCircle2 size={13} />
            Key configured
          </p>
        )}

        {/* Key input — type="password" masks the value on screen (T-11-09) */}
        <div className="flex gap-2">
          <input
            type="password"
            value={ytKeyInput}
            onChange={(e) => setYtKeyInput(e.target.value)}
            placeholder="AIza..."
            autoComplete="off"
            className="flex-1 rounded-lg border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                void handleSaveYouTubeKey();
              }
            }}
          />
          <button
            type="button"
            onClick={() => void handleSaveYouTubeKey()}
            disabled={loading || !ytKeyInput.trim()}
            className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? <Loader2 size={14} className="animate-spin" /> : null}
            Save key
          </button>
        </div>

        {saved && (
          <p className="text-xs text-emerald-500">Key saved. Related videos will appear on next lesson visit.</p>
        )}

        {removed && (
          <p data-testid="yt-key-removed-msg" className="text-xs text-muted-foreground">
            YouTube key removed.
          </p>
        )}

        {error && (
          <p className="text-xs text-destructive">{error}</p>
        )}

        {/* Remove key — only shown when a key is currently configured */}
        {configured && (
          <div className="border-t border-border pt-3">
            <button
              type="button"
              onClick={() => void handleRemoveYouTubeKey()}
              disabled={loading}
              className="text-xs text-muted-foreground underline underline-offset-2 transition-colors hover:text-destructive disabled:opacity-50"
            >
              Remove YouTube key
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
