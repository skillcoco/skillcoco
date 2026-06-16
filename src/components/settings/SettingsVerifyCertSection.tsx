// Phase 6 (Certification) — Plan 06-06 (Wave 5) Settings Verify panel.
//
// Per D-05 + CERT-04..05 + CERT-08: paste a base64 QR payload, click
// Verify, see signer fingerprint + decoded fields. Optional override PEM
// lets the learner validate someone else's cert offline. "Show signing
// public key" copies the local PEM to clipboard for sharing.
//
// W4 fix: mount-time `useEffect` populates `localFingerprint` by calling
// `getSigningPublicKey()` → `fingerprintFromPublicPem(pem)`. The
// untrusted-signer warning fires on the FIRST override paste — no prior
// verify pass required. Mount errors degrade silently to
// `localFingerprint = null` (the verifier still works; only the warning
// is suppressed until a key exists). React-strict-mode-safe via a
// cancellation flag.
//
// Glass + lucide-icons only; no emoji per D-08.

import { useEffect, useState } from "react";
import { AlertTriangle, CheckCircle2, Clipboard, XCircle } from "lucide-react";
import * as commands from "@/lib/tauri-commands";
import type { VerifySignatureResult } from "@/types/achievements";

// ── Error-code → friendly message mapping ────────────────────────────

const ERROR_MAP: Record<string, string> = {
  signature_mismatch: "Signature does not match the public key.",
  malformed_envelope:
    "Payload format is invalid (expected base64 followed by a dot and signature hex).",
  invalid_base64: "Payload base64 could not be decoded.",
  payload_too_large: "Payload exceeds the 8 KB safety cap.",
  public_key_too_large: "Public key PEM exceeds the 4 KB safety cap.",
  local_public_key_unavailable:
    "No local signing key exists yet. Generate a certificate first or paste a public key override.",
  payload_unparseable: "Payload fields could not be parsed.",
};

function friendlyError(code: string | null): string {
  if (!code) return "";
  return ERROR_MAP[code] ?? `Verification failed: ${code}`;
}

function isNoKeyError(err: unknown): boolean {
  const s = String(err ?? "").toLowerCase();
  return (
    s.includes("not found") ||
    s.includes("no such file") ||
    s.includes("os error 2") ||
    s.includes("io")
  );
}

// ── Component ────────────────────────────────────────────────────────

export function SettingsVerifyCertSection() {
  const [payload, setPayload] = useState("");
  const [override, setOverride] = useState("");
  const [showOverride, setShowOverride] = useState(false);
  const [result, setResult] = useState<VerifySignatureResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [localFingerprint, setLocalFingerprint] = useState<string | null>(null);
  const [copyMsg, setCopyMsg] = useState<string | null>(null);
  const [noKeyMsg, setNoKeyMsg] = useState<string | null>(null);

  // ── W4 fix: derive localFingerprint on mount. ────────────────────
  // Sequence:
  //   1. Call getSigningPublicKey() → PEM (rejects on cold-start).
  //   2. Call fingerprintFromPublicPem(pem) → 8-hex string.
  //   3. Store in state. Any failure leaves it at null (silent).
  // The cancellation flag survives React strict-mode double-mount so
  // a late-arriving promise from the first mount doesn't clobber the
  // second mount's state.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const pem = await commands.getSigningPublicKey();
        if (cancelled) return;
        try {
          const fp = await commands.fingerprintFromPublicPem(pem);
          if (cancelled) return;
          setLocalFingerprint(fp);
        } catch {
          // Malformed PEM on disk — silent degrade.
        }
      } catch {
        // No key yet (Phase 6 generates lazily) — silent degrade.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Auto-dismiss "Copied" confirmation after 2 seconds.
  useEffect(() => {
    if (!copyMsg) return;
    const t = setTimeout(() => setCopyMsg(null), 2000);
    return () => clearTimeout(t);
  }, [copyMsg]);

  // ── Handlers ─────────────────────────────────────────────────────

  async function onVerify() {
    setBusy(true);
    setNoKeyMsg(null);
    try {
      const overrideTrimmed = showOverride && override.trim().length > 0
        ? override
        : null;
      const r = await commands.verifySignature({
        payloadB64: payload,
        publicKeyPemOverride: overrideTrimmed,
      });
      setResult(r);
    } catch (e) {
      setResult({
        valid: false,
        learner: "",
        track: "",
        level: "",
        completionDate: "",
        keyFingerprint: "",
        payloadVersion: 0,
        error: String(e),
      });
    } finally {
      setBusy(false);
    }
  }

  async function onCopyPublicKey() {
    setNoKeyMsg(null);
    setCopyMsg(null);
    try {
      // Re-call getSigningPublicKey at click time (the key may have been
      // generated between mount and this click, e.g. the learner earned
      // their first achievement after opening Settings).
      const pem = await commands.getSigningPublicKey();
      await navigator.clipboard.writeText(pem);
      setCopyMsg("Copied");
    } catch (e) {
      if (isNoKeyError(e)) {
        setNoKeyMsg(
          "Generate a certificate first to create a signing key.",
        );
      } else {
        setNoKeyMsg(`Could not copy public key: ${String(e)}`);
      }
    }
  }

  // ── Derived ──────────────────────────────────────────────────────

  const untrustedSigner =
    showOverride &&
    override.trim().length > 0 &&
    result !== null &&
    result.valid &&
    localFingerprint !== null &&
    result.keyFingerprint.length > 0 &&
    result.keyFingerprint !== localFingerprint;

  // ── Render ───────────────────────────────────────────────────────

  return (
    <section className="space-y-4" aria-label="Verify certificate">
      {/* Hidden probe — test-only seam for the mount-time fingerprint. */}
      <span data-testid="local-fp" hidden>
        {localFingerprint ?? ""}
      </span>

      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-foreground">
          Verify certificate
        </h2>
        <button
          type="button"
          onClick={onCopyPublicKey}
          className="flex items-center gap-1.5 rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
        >
          <Clipboard size={12} />
          Show signing public key
        </button>
      </div>

      <div className="glass space-y-4 rounded-xl p-5">
        <p className="text-xs leading-relaxed text-muted-foreground">
          Paste a base64 payload from a QR code to verify its signature
          against the local signing key, or against a public key you paste
          below. Note: your name appears inside the QR code. Update your
          name in Profile if needed.
        </p>

        <div>
          <label
            htmlFor="verify-payload"
            className="mb-1.5 block text-xs font-medium text-foreground"
          >
            Payload
          </label>
          <textarea
            id="verify-payload"
            value={payload}
            onChange={(e) => setPayload(e.target.value)}
            placeholder="Paste base64 payload from QR code"
            rows={4}
            className="w-full rounded-md border border-input bg-background px-3 py-2 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
          />
        </div>

        <div>
          <button
            type="button"
            onClick={() => setShowOverride((v) => !v)}
            className="text-xs text-primary hover:underline"
          >
            {showOverride ? "Hide" : "Use a different public key"}
          </button>
          {showOverride && (
            <div className="mt-2">
              <label
                htmlFor="verify-override"
                className="mb-1.5 block text-xs font-medium text-foreground"
              >
                Public key PEM override
              </label>
              <textarea
                id="verify-override"
                value={override}
                onChange={(e) => setOverride(e.target.value)}
                placeholder={"-----BEGIN PUBLIC KEY-----\n...\n-----END PUBLIC KEY-----"}
                rows={4}
                className="w-full rounded-md border border-input bg-background px-3 py-2 text-xs font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              />
            </div>
          )}
        </div>

        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={onVerify}
            disabled={payload.trim().length === 0 || busy}
            className="rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {busy ? "Verifying..." : "Verify"}
          </button>
          {copyMsg && (
            <span className="text-xs font-medium text-emerald-500">
              {copyMsg}
            </span>
          )}
          {noKeyMsg && (
            <span role="alert" className="text-xs text-amber-500">
              {noKeyMsg}
            </span>
          )}
        </div>

        {/* Result block */}
        {result && result.valid && (
          <div
            data-testid="verify-result-valid"
            className="space-y-2 rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-4"
          >
            <div className="flex items-center gap-2 text-sm font-medium text-emerald-500">
              <CheckCircle2 size={16} />
              Valid signature
            </div>
            <dl className="grid grid-cols-1 gap-x-4 gap-y-1 text-xs sm:grid-cols-2">
              <div className="flex gap-2">
                <dt className="text-muted-foreground">Learner</dt>
                <dd className="font-medium text-foreground">{result.learner}</dd>
              </div>
              <div className="flex gap-2">
                <dt className="text-muted-foreground">Track</dt>
                <dd className="font-medium text-foreground">{result.track}</dd>
              </div>
              <div className="flex gap-2">
                <dt className="text-muted-foreground">Level</dt>
                <dd className="font-medium text-foreground">{result.level}</dd>
              </div>
              <div className="flex gap-2">
                <dt className="text-muted-foreground">Date</dt>
                <dd className="font-medium text-foreground">
                  {result.completionDate}
                </dd>
              </div>
              <div className="flex gap-2 sm:col-span-2">
                <dt className="text-muted-foreground">Signer fingerprint</dt>
                <dd className="font-mono font-medium text-foreground">
                  {result.keyFingerprint}
                </dd>
              </div>
            </dl>
            {untrustedSigner && (
              <div
                role="alert"
                className="mt-2 flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2"
              >
                <AlertTriangle
                  size={14}
                  className="mt-0.5 shrink-0 text-amber-500"
                />
                <p className="text-xs leading-relaxed text-amber-400">
                  Verifying against external key: {result.keyFingerprint}. This
                  signer is not your local LearnForge install.
                </p>
              </div>
            )}
          </div>
        )}

        {result && !result.valid && (
          <div
            data-testid="verify-result-invalid"
            role="alert"
            className="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/10 p-4"
          >
            <XCircle size={16} className="mt-0.5 shrink-0 text-destructive" />
            <div className="text-xs leading-relaxed text-destructive">
              {friendlyError(result.error)}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
