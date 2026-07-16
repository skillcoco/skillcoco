// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)
//
// Phase 6 (Certification) — Plan 06-06 (Wave 5) SettingsVerifyCertSection tests.
// Phase 13 (OSS Consolidation) — migrated from pro/ to src/pages/ per D-08.
//
// Per the W4 fix: the section MUST populate `localFingerprint` automatically
// on mount by calling `getSigningPublicKey()` → `fingerprintFromPublicPem(pem)`,
// NOT lazily after a successful verify. This guarantees the untrusted-signer
// warning works on the FIRST override paste (acceptance walkthrough step 10).
//
// Mocking strategy: only `@/lib/tauri-commands` is mocked. `vi.hoisted` is
// used so the mock references survive vi.mock factory hoisting.

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { VerifySignatureResult } from "@/types/achievements";

// ── Mocks ────────────────────────────────────────────────────────────

const {
  verifySignatureMock,
  getSigningPublicKeyMock,
  fingerprintFromPublicPemMock,
  pickAndReadReportFileMock,
} = vi.hoisted(() => ({
  verifySignatureMock: vi.fn(),
  getSigningPublicKeyMock: vi.fn(),
  fingerprintFromPublicPemMock: vi.fn(),
  pickAndReadReportFileMock: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  verifySignature: verifySignatureMock,
  getSigningPublicKey: getSigningPublicKeyMock,
  fingerprintFromPublicPem: fingerprintFromPublicPemMock,
  pickAndReadReportFile: pickAndReadReportFileMock,
}));

import { SettingsVerifyCertSection } from "@/pages/SettingsVerifyCertSection";

// ── Helpers ──────────────────────────────────────────────────────────

const SAMPLE_PEM =
  "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAabcd1234abcd1234abcd1234abcd1234abcd1234abcd\n-----END PUBLIC KEY-----\n";

function okResult(overrides: Partial<VerifySignatureResult> = {}): VerifySignatureResult {
  return {
    valid: true,
    learner: "Ada",
    track: "Kubernetes",
    level: "Associate",
    completionDate: "2026-06-15T00:00:00Z",
    keyFingerprint: "deadbeef",
    payloadVersion: 1,
    error: null,
    ...overrides,
  };
}

function badResult(overrides: Partial<VerifySignatureResult> = {}): VerifySignatureResult {
  return {
    valid: false,
    learner: "",
    track: "",
    level: "",
    completionDate: "",
    keyFingerprint: "",
    payloadVersion: 0,
    error: "signature_mismatch",
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  // Default: no key yet (cold-start). Each test that needs a key opts in.
  getSigningPublicKeyMock.mockRejectedValue(new Error("not found"));
  fingerprintFromPublicPemMock.mockResolvedValue("deadbeef");
  verifySignatureMock.mockResolvedValue(okResult());
  // Stub clipboard.
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
    writable: true,
    configurable: true,
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SettingsVerifyCertSection — Phase 6 Plan 06-06 (Wave 5)", () => {
  it("renders_paste_textarea_and_verify_button", async () => {
    render(<SettingsVerifyCertSection />);

    expect(screen.getByLabelText(/payload/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^verify$/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /show signing public key/i }),
    ).toBeInTheDocument();
    // One-time inline disclosure per RESEARCH.md privacy note.
    expect(screen.getByText(/your name appears inside the qr/i)).toBeInTheDocument();
  });

  it("verify_button_disabled_when_input_empty", () => {
    render(<SettingsVerifyCertSection />);
    const btn = screen.getByRole("button", { name: /^verify$/i });
    expect(btn).toBeDisabled();
  });

  it("populates_localFingerprint_on_mount", async () => {
    getSigningPublicKeyMock.mockResolvedValue(SAMPLE_PEM);
    fingerprintFromPublicPemMock.mockResolvedValue("a1b2c3d4");

    render(<SettingsVerifyCertSection />);

    await waitFor(() => {
      expect(getSigningPublicKeyMock).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(fingerprintFromPublicPemMock).toHaveBeenCalledWith(SAMPLE_PEM);
    });
    await waitFor(() => {
      expect(screen.getByTestId("local-fp")).toHaveTextContent("a1b2c3d4");
    });
  });

  it("mount_silently_handles_missing_key", async () => {
    getSigningPublicKeyMock.mockRejectedValue(new Error("not found"));

    render(<SettingsVerifyCertSection />);

    // Wait for the mount-time IPC chain to settle.
    await waitFor(() => {
      expect(getSigningPublicKeyMock).toHaveBeenCalledTimes(1);
    });
    // fingerprintFromPublicPem must NEVER be called when getSigningPublicKey fails.
    expect(fingerprintFromPublicPemMock).not.toHaveBeenCalled();
    // No error UI shown (silent degradation).
    expect(screen.queryByRole("alert", { name: /signing key/i })).toBeNull();
    expect(screen.queryByText(/failed to load signing key/i)).toBeNull();
    // The verifier UI still renders.
    expect(
      screen.getByRole("button", { name: /^verify$/i }),
    ).toBeInTheDocument();
    // Local fingerprint probe is empty.
    expect(screen.getByTestId("local-fp")).toHaveTextContent("");
  });

  it("mount_silently_handles_malformed_pem", async () => {
    getSigningPublicKeyMock.mockResolvedValue("garbage");
    fingerprintFromPublicPemMock.mockRejectedValue(new Error("decode public pem"));

    render(<SettingsVerifyCertSection />);

    await waitFor(() => {
      expect(fingerprintFromPublicPemMock).toHaveBeenCalledWith("garbage");
    });
    // No error UI shown (silent degradation).
    expect(screen.queryByText(/failed to load signing key/i)).toBeNull();
    expect(screen.getByTestId("local-fp")).toHaveTextContent("");
  });

  it("verify_button_calls_ipc_with_pasted_value", async () => {
    render(<SettingsVerifyCertSection />);
    const ta = screen.getByLabelText(/payload/i);
    await userEvent.type(ta, "abc.def");

    const btn = screen.getByRole("button", { name: /^verify$/i });
    expect(btn).not.toBeDisabled();
    await userEvent.click(btn);

    await waitFor(() => {
      expect(verifySignatureMock).toHaveBeenCalledWith({
        payloadB64: "abc.def",
        publicKeyPemOverride: null,
      });
    });
  });

  it("shows_valid_result_with_parsed_fields", async () => {
    verifySignatureMock.mockResolvedValue(
      okResult({
        learner: "Ada",
        track: "Kubernetes",
        level: "Associate",
        completionDate: "2026-06-15T00:00:00Z",
        keyFingerprint: "a1b2c3d4",
      }),
    );

    render(<SettingsVerifyCertSection />);
    await userEvent.type(screen.getByLabelText(/payload/i), "abc.def");
    await userEvent.click(screen.getByRole("button", { name: /^verify$/i }));

    await waitFor(() => {
      expect(screen.getByTestId("verify-result-valid")).toBeInTheDocument();
    });
    expect(screen.getByText(/ada/i)).toBeInTheDocument();
    expect(screen.getByText(/kubernetes/i)).toBeInTheDocument();
    expect(screen.getByText(/associate/i)).toBeInTheDocument();
    expect(screen.getByText(/a1b2c3d4/i)).toBeInTheDocument();
  });

  it("shows_invalid_result_with_error_message", async () => {
    verifySignatureMock.mockResolvedValue(
      badResult({ error: "signature_mismatch" }),
    );

    render(<SettingsVerifyCertSection />);
    await userEvent.type(screen.getByLabelText(/payload/i), "abc.def");
    await userEvent.click(screen.getByRole("button", { name: /^verify$/i }));

    await waitFor(() => {
      expect(screen.getByTestId("verify-result-invalid")).toBeInTheDocument();
    });
    expect(
      screen.getByText(/signature does not match/i),
    ).toBeInTheDocument();
  });

  it("supports_optional_public_key_override", async () => {
    render(<SettingsVerifyCertSection />);
    await userEvent.type(screen.getByLabelText(/payload/i), "abc.def");

    // Reveal the override panel.
    await userEvent.click(
      screen.getByRole("button", { name: /use a different public key/i }),
    );
    const overrideTa = screen.getByLabelText(/public key pem override/i);
    await userEvent.type(overrideTa, SAMPLE_PEM);

    await userEvent.click(screen.getByRole("button", { name: /^verify$/i }));

    await waitFor(() => {
      expect(verifySignatureMock).toHaveBeenCalledWith({
        payloadB64: "abc.def",
        publicKeyPemOverride: SAMPLE_PEM,
      });
    });
  });

  it("shows_untrusted_signer_warning_when_fingerprints_differ_on_first_paste", async () => {
    // Mount-time: local fingerprint is "aaaaaaaa".
    getSigningPublicKeyMock.mockResolvedValue(SAMPLE_PEM);
    fingerprintFromPublicPemMock.mockResolvedValue("aaaaaaaa");
    // Verify returns a different signer fingerprint.
    verifySignatureMock.mockResolvedValue(
      okResult({ keyFingerprint: "bbbbbbbb" }),
    );

    render(<SettingsVerifyCertSection />);
    // Wait for mount-time fingerprint derivation.
    await waitFor(() => {
      expect(screen.getByTestId("local-fp")).toHaveTextContent("aaaaaaaa");
    });

    // User pastes an override PEM on FIRST interaction (no prior verify).
    await userEvent.type(screen.getByLabelText(/payload/i), "abc.def");
    await userEvent.click(
      screen.getByRole("button", { name: /use a different public key/i }),
    );
    await userEvent.type(
      screen.getByLabelText(/public key pem override/i),
      SAMPLE_PEM,
    );
    await userEvent.click(screen.getByRole("button", { name: /^verify$/i }));

    await waitFor(() => {
      expect(screen.getByTestId("verify-result-valid")).toBeInTheDocument();
    });
    // Untrusted signer warning fires on the FIRST override paste.
    expect(
      screen.getByText(/verifying against external key/i),
    ).toBeInTheDocument();
  });

  it("show_public_key_button_copies_to_clipboard", async () => {
    getSigningPublicKeyMock.mockResolvedValue(SAMPLE_PEM);
    fingerprintFromPublicPemMock.mockResolvedValue("deadbeef");
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      writable: true,
      configurable: true,
    });

    render(<SettingsVerifyCertSection />);

    await userEvent.click(
      screen.getByRole("button", { name: /show signing public key/i }),
    );

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(SAMPLE_PEM);
    });
    expect(screen.getByText(/^copied$/i)).toBeInTheDocument();

    // The 2s auto-dismiss is tested with real timers via waitFor + 2.5s
    // timeout so we exercise the actual setTimeout path without fake-timer
    // interaction races (userEvent + advanceTimers had reliability issues).
    await waitFor(
      () => {
        expect(screen.queryByText(/^copied$/i)).toBeNull();
      },
      { timeout: 2500 },
    );
  }, 10000);

  it("show_public_key_handles_no_key_yet_gracefully", async () => {
    getSigningPublicKeyMock.mockRejectedValue(new Error("not found"));

    render(<SettingsVerifyCertSection />);

    await userEvent.click(
      screen.getByRole("button", { name: /show signing public key/i }),
    );

    await waitFor(() => {
      expect(
        screen.getByText(/generate a certificate first to create a signing key/i),
      ).toBeInTheDocument();
    });
  });

  it("no_emoji_in_rendered_output", async () => {
    const { container } = render(<SettingsVerifyCertSection />);
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /^verify$/i }),
      ).toBeInTheDocument();
    });
    const text = container.textContent ?? "";
    const emojiRegex = /[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u;
    expect(text).not.toMatch(emojiRegex);
  });

  // ── Phase 18 (18-06 UAT) — heading covers reports; file-pick verify ──

  it("heading reads 'Verify certificate or report' (reports are first-class here)", () => {
    render(<SettingsVerifyCertSection />);
    expect(
      screen.getByRole("heading", { name: "Verify certificate or report" }),
    ).toBeInTheDocument();
  });

  it("choosing a certificate file loads it and verifies without pasting", async () => {
    const fileContent =
      '{"payload":{"learner":"Ada"},"signatureHex":"aa","keyFingerprint":"deadbeef"}';
    pickAndReadReportFileMock.mockResolvedValue(fileContent);
    verifySignatureMock.mockResolvedValue(
      okResult({ learner: "Ada", track: "Kubernetes", keyFingerprint: "deadbeef" }),
    );
    const user = userEvent.setup();
    render(<SettingsVerifyCertSection />);

    await user.click(
      screen.getByRole("button", { name: /choose report file/i }),
    );

    await waitFor(() => {
      expect(verifySignatureMock).toHaveBeenCalledTimes(1);
    });
    expect(verifySignatureMock).toHaveBeenCalledWith({
      payloadB64: fileContent,
      publicKeyPemOverride: null,
    });
    expect(
      await screen.findByTestId("verify-result-valid"),
    ).toBeInTheDocument();
  });

  it("cancelling the file picker does not verify", async () => {
    pickAndReadReportFileMock.mockResolvedValue(null);
    const user = userEvent.setup();
    render(<SettingsVerifyCertSection />);

    await user.click(
      screen.getByRole("button", { name: /choose report file/i }),
    );

    await waitFor(() => {
      expect(pickAndReadReportFileMock).toHaveBeenCalledTimes(1);
    });
    expect(verifySignatureMock).not.toHaveBeenCalled();
  });
});
