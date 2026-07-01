// Phase 11 UAT fix — SettingsYouTubeSection configured-state feedback.
//
// RED tests (written before implementation):
//   - configured indicator visible when key is stored
//   - Remove link visible only when configured
//   - clicking Remove shows "removed" confirmation + hides indicator + hides Remove link
//   - configured false on mount → no Remove link, no indicator
//   - saving a key → configured indicator appears

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// Hoist mocks before any import that might import the real module
const { isYoutubeKeyConfiguredMock, loginProviderMock, logoutProviderMock } = vi.hoisted(
  () => ({
    isYoutubeKeyConfiguredMock: vi.fn(),
    loginProviderMock: vi.fn(),
    logoutProviderMock: vi.fn(),
  }),
);

vi.mock("@/lib/tauri-commands", () => ({
  isYoutubeKeyConfigured: isYoutubeKeyConfiguredMock,
  loginProvider: loginProviderMock,
  logoutProvider: logoutProviderMock,
}));

import { SettingsYouTubeSection } from "@/pages/SettingsYouTubeSection";

describe("SettingsYouTubeSection — configured-state feedback", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows configured indicator and Remove link when key is stored on mount", async () => {
    isYoutubeKeyConfiguredMock.mockResolvedValue(true);
    render(<SettingsYouTubeSection />);

    await waitFor(() => {
      expect(screen.getByTestId("yt-key-configured-indicator")).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /remove youtube key/i })).toBeInTheDocument();
  });

  it("hides configured indicator and Remove link when no key on mount", async () => {
    isYoutubeKeyConfiguredMock.mockResolvedValue(false);
    render(<SettingsYouTubeSection />);

    // Wait for the async mount check to settle
    await waitFor(() => {
      expect(isYoutubeKeyConfiguredMock).toHaveBeenCalledOnce();
    });
    expect(screen.queryByTestId("yt-key-configured-indicator")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /remove youtube key/i })).not.toBeInTheDocument();
  });

  it("shows removed confirmation and hides configured indicator after Remove click", async () => {
    isYoutubeKeyConfiguredMock.mockResolvedValue(true);
    logoutProviderMock.mockResolvedValue(undefined);
    render(<SettingsYouTubeSection />);

    // Wait for configured state to load
    await waitFor(() => {
      expect(screen.getByTestId("yt-key-configured-indicator")).toBeInTheDocument();
    });

    const removeBtn = screen.getByRole("button", { name: /remove youtube key/i });
    await userEvent.click(removeBtn);

    await waitFor(() => {
      expect(screen.getByTestId("yt-key-removed-msg")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("yt-key-configured-indicator")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /remove youtube key/i })).not.toBeInTheDocument();
    expect(logoutProviderMock).toHaveBeenCalledWith("youtube");
  });

  it("sets configured indicator after saving a new key", async () => {
    isYoutubeKeyConfiguredMock.mockResolvedValue(false);
    loginProviderMock.mockResolvedValue({ provider: "youtube", authenticated: true });
    render(<SettingsYouTubeSection />);

    await waitFor(() => {
      expect(isYoutubeKeyConfiguredMock).toHaveBeenCalledOnce();
    });

    const input = screen.getByPlaceholderText(/AIza/i);
    await userEvent.type(input, "AIza-new-test-key");

    const saveBtn = screen.getByRole("button", { name: /save key/i });
    await userEvent.click(saveBtn);

    await waitFor(() => {
      expect(screen.getByTestId("yt-key-configured-indicator")).toBeInTheDocument();
    });
    expect(loginProviderMock).toHaveBeenCalledWith({
      provider: "youtube",
      method: "api-key",
      credential: "AIza-new-test-key",
    });
  });

  it("treats IPC error on mount as not-configured (fail-soft)", async () => {
    isYoutubeKeyConfiguredMock.mockRejectedValue(new Error("IPC unavailable"));
    render(<SettingsYouTubeSection />);

    await waitFor(() => {
      expect(isYoutubeKeyConfiguredMock).toHaveBeenCalledOnce();
    });
    // Must not crash and must not show the configured indicator
    expect(screen.queryByTestId("yt-key-configured-indicator")).not.toBeInTheDocument();
  });
});
