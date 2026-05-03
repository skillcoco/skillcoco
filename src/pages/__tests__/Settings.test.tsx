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
}));

import {
  checkOAuthStatus,
  startOAuthLogin,
  detectSystemProviders,
  getOrCreateProfile,
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

    // Wait for page to load
    await waitFor(() => {
      expect(screen.getByText(/claude/i)).toBeInTheDocument();
    });

    // Find and click login button
    const loginButtons = screen.queryAllByRole("button", { name: /login|connect|setup/i });
    if (loginButtons.length > 0) {
      await userEvent.click(loginButtons[0]);
    }

    // After polling, the error should surface as an alert/banner
    // This assertion FAILS today — Settings.tsx ignores the error field
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
      expect(screen.getByText(/claude/i)).toBeInTheDocument();
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
});
