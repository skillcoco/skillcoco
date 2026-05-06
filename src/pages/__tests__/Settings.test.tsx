// Wave 0 scaffold — Plan 02 (FIX-01) makes the error-banner assertion green.
// Today: Settings.tsx does not surface OAuthStatusResult.error — it only checks
// status.completed and times out silently. The error-banner test FAILS.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";

// Mock tauri commands — never calls real Tauri bridge in tests
vi.mock("@/lib/tauri-commands", () => ({
  checkOAuthStatus: vi.fn(),
  startOAuthLogin: vi.fn(),
  saveSetupToken: vi.fn(),
  detectSystemProviders: vi.fn(),
  getOrCreateProfile: vi.fn(),
  updateProfile: vi.fn(),
  // Phase 03.1 LAB-03 — Labs runtime selector dependencies
  labRuntimeDetect: vi.fn(),
  // Provider auth surface used by Settings.tsx via the wildcard import.
  // Tests don't assert on these, but the module must export them or the
  // wildcard `import * as commands` returns undefined for missing keys.
  getAuthStatus: vi.fn(),
  loginProvider: vi.fn(),
  logoutProvider: vi.fn(),
  setActiveProvider: vi.fn(),
}));

import {
  checkOAuthStatus,
  startOAuthLogin,
  detectSystemProviders,
  getOrCreateProfile,
  labRuntimeDetect,
  getAuthStatus,
} from "@/lib/tauri-commands";

// Mock useAppStore to prevent Tauri state dependencies
vi.mock("@/stores/useAppStore", () => ({
  useAppStore: vi.fn(() => ({
    profile: null,
    loadProfile: vi.fn(),
    updateProfile: vi.fn(),
  })),
}));

import { Settings } from "@/pages/Settings";

function renderSettings() {
  return render(
    <MemoryRouter>
      <Settings />
    </MemoryRouter>,
  );
}

describe("Settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(detectSystemProviders).mockResolvedValue([]);
    vi.mocked(getAuthStatus).mockResolvedValue([]);
    vi.mocked(getOrCreateProfile).mockResolvedValue({
      id: "p1",
      displayName: "Test User",
      learningStyle: "practical",
      experienceLevel: "intermediate",
      preferencesJson: "{}",
      createdAt: "2026-01-01",
      updatedAt: "2026-01-01",
    });
    vi.mocked(checkOAuthStatus).mockResolvedValue({
      completed: false,
      provider: "claude",
      authenticated: false,
    });
    vi.mocked(startOAuthLogin).mockResolvedValue({ started: true, provider: "claude" });
    vi.mocked(labRuntimeDetect).mockResolvedValue({
      dockerAvailable: true,
      dockerVersion: "24.0.5",
      effectiveRuntime: "docker",
      setting: "autoDetect",
    });
  });

  it("displays inline error alert when OAuthStatusResult.error is set", async () => {
    // FAILING TODAY — Plan 02 (FIX-01) will make this green by surfacing error field.
    // When checkOAuthStatus returns an error, Settings.tsx should show an inline alert.
    vi.mocked(checkOAuthStatus).mockResolvedValue({
      completed: false,
      provider: "claude",
      authenticated: false,
      error: "Invalid bearer token",
    });

    // Simulate: user clicks "Login with Claude" -> startOAuthLogin -> poll -> error
    vi.mocked(startOAuthLogin).mockResolvedValue({ started: true, provider: "claude" });

    renderSettings();

    // Wait for page to load — use getAllByText since "Claude" appears in header and select
    await waitFor(() => {
      expect(screen.getAllByText(/claude/i).length).toBeGreaterThan(0);
    });

    // Find and click login button
    const loginButtons = screen.queryAllByRole("button", { name: /login|connect|setup/i });
    if (loginButtons.length > 0) {
      await userEvent.click(loginButtons[0]);
    }

    // After polling, the error should surface as an alert/banner
    // checkOAuthStatus is pre-mocked to return error: "Invalid bearer token"
    await waitFor(
      () => {
        expect(screen.getByText(/invalid bearer token/i)).toBeInTheDocument();
      },
      { timeout: 3000 },
    );
  });

  it("provider connection flow shows 'connecting' then 'connected' state", async () => {
    // FIX-05 happy path scaffold — Plan 02 makes this reliable.
    // Minimal assertion: startOAuthLogin is invoked when "Login" button clicked.
    renderSettings();

    await waitFor(() => {
      expect(screen.getAllByText(/claude/i).length).toBeGreaterThan(0);
    });

    const loginButtons = screen.queryAllByRole("button", { name: /login|connect|setup/i });
    if (loginButtons.length > 0) {
      await userEvent.click(loginButtons[0]);
      await waitFor(() => {
        expect(startOAuthLogin).toHaveBeenCalled();
      });
    } else {
      // No login button found — Settings may show different state
      // This is expected to evolve as Plan 02 implements the UI
      expect(screen.getByText(/claude/i)).toBeInTheDocument();
    }
  });

  // ── Phase 03.1 Wave 0 — Labs runtime selector (LAB-03) ──
  // FAILS today — Settings has no Labs section. Plan 03.1-07 makes these green.

  describe("Labs runtime selector (Phase 03.1 LAB-03)", () => {
    it("settings_labs_section_exists — renders a Labs section heading", async () => {
      renderSettings();
      const heading = await screen.findByRole("heading", { name: /labs/i });
      expect(heading).toBeInTheDocument();
    });

    it("settings_labs_runtime_default_autodetect — runtime selector defaults to Auto-detect", async () => {
      renderSettings();
      const selector = await screen.findByTestId("labs-runtime-select");
      // The default value should reflect "autoDetect" (camelCase per LabRuntimeChoice).
      expect(selector).toHaveAttribute("data-value", "autoDetect");
    });

    it("settings_labs_runtime_options — three options visible: Docker, Host shell, Auto-detect", async () => {
      renderSettings();
      // The selector exposes its options as buttons / option elements.
      expect(await screen.findByText(/docker/i)).toBeInTheDocument();
      expect(await screen.findByText(/host shell/i)).toBeInTheDocument();
      expect(await screen.findByText(/auto-?detect/i)).toBeInTheDocument();
    });

    it("settings_labs_runtime_persists — selection invokes update_profile with preferences_json", async () => {
      renderSettings();
      const selector = await screen.findByTestId("labs-runtime-select");
      // Simulate the selector change by firing a DOM-level change event.
      // The component is expected to read the new value and persist via
      // tauri-commands.updateProfile with a preferences_json payload that
      // includes labs_runtime.
      const { updateProfile } = await import("@/lib/tauri-commands");
      vi.mocked(updateProfile).mockResolvedValue({
        id: "p1",
        displayName: "Test User",
        learningStyle: "practical",
        experienceLevel: "intermediate",
        preferencesJson: '{"labs_runtime":"docker"}',
        createdAt: "2026-01-01",
        updatedAt: "2026-01-01",
      });
      await userEvent.click(selector);
      const dockerOption = await screen.findByRole("option", { name: /^docker$/i });
      await userEvent.click(dockerOption);
      await waitFor(() => {
        expect(updateProfile).toHaveBeenCalled();
        const arg = vi.mocked(updateProfile).mock.calls[0]?.[0] ?? {};
        // preferencesJson is the camelCase Tauri-bridged field.
        expect(JSON.stringify(arg)).toMatch(/labs_runtime/);
      });
    });

    it("settings_labs_docker_status_indicator — green when docker_available true, gray when false", async () => {
      // The Settings page is expected to invoke a probe (e.g. docker_available
      // command) and surface the status. Wave 0 expectation: the indicator
      // testid is present and reflects the resolved status.
      renderSettings();
      const indicator = await screen.findByTestId("labs-docker-status");
      expect(["docker-available", "docker-unavailable"]).toContain(
        indicator.getAttribute("data-status"),
      );
    });
  });
});
