import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

// ── Mock Tauri IPC ─────────────────────────────────────────────────────────────
// Must be hoisted before the component import so the mock is in place when
// the module initialises. Uses vi.hoisted for the function references so they
// can be reassigned per-test.

const mockGetLessonVideos = vi.hoisted(() => vi.fn());
const mockRefreshLessonVideos = vi.hoisted(() => vi.fn());
const mockIsYoutubeKeyConfigured = vi.hoisted(() => vi.fn());

vi.mock("@/lib/tauri-commands", () => ({
  getLessonVideos: mockGetLessonVideos,
  refreshLessonVideos: mockRefreshLessonVideos,
  isYoutubeKeyConfigured: mockIsYoutubeKeyConfigured,
}));

import { ReferenceVideoPanel } from "../ReferenceVideoPanel";
import type { LessonVideosResult } from "@/types/videos";

// ── Helpers ────────────────────────────────────────────────────────────────────

const VIDEO_A = {
  videoId: "vidA",
  title: "Pod Lifecycle Explained",
  channelTitle: "KubeLearner",
  relevanceScore: 0.85,
};

function makeResult(videos: typeof VIDEO_A[]): LessonVideosResult {
  return { videos };
}

const DEFAULT_PROPS = {
  moduleId: "mod-1",
  sectionId: "sec-1",
  sectionTitle: "Pod Lifecycle",
};

// ── Tests ──────────────────────────────────────────────────────────────────────

describe("ReferenceVideoPanel (acceptance)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: no YouTube key configured — preserves D-06 behaviour for all
    // existing tests that do not explicitly set up the key mock.
    mockIsYoutubeKeyConfigured.mockResolvedValue(false);
  });

  // ── D-09: null-on-empty ────────────────────────────────────────────────────

  it("ref_vid_null_on_empty — renders nothing when backend returns empty list", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([]));

    const { container } = render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(mockGetLessonVideos).toHaveBeenCalledTimes(1);
    });

    // D-09: no element should be in the DOM
    expect(container.firstChild).toBeNull();
    expect(screen.queryByTestId("reference-video-panel")).toBeNull();
  });

  it("ref_vid_null_on_error — renders nothing when IPC call throws", async () => {
    mockGetLessonVideos.mockRejectedValue(new Error("quota exceeded"));

    const { container } = render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(mockGetLessonVideos).toHaveBeenCalledTimes(1);
    });

    expect(container.firstChild).toBeNull();
  });

  // ── Single video render ────────────────────────────────────────────────────

  it("ref_vid_renders_single_video — panel shows when backend returns one video", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });

    // Title and channel must be visible
    expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    expect(screen.getByText(VIDEO_A.channelTitle)).toBeInTheDocument();
  });

  it("ref_vid_only_first_video — takes first video when backend (incorrectly) returns multiple", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Another Pod Video" };
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A, VIDEO_B]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });

    // Only the first video's title should appear
    expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    expect(screen.queryByText(VIDEO_B.title)).not.toBeInTheDocument();
  });

  // ── Replace button ────────────────────────────────────────────────────────

  it("ref_vid_replace_calls_refresh_with_exclude_id — Replace calls refreshLessonVideos with current video id", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Different Pod Video" };
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([VIDEO_B]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    const replaceBtn = screen.getByRole("button", { name: /replace with a different video/i });
    await user.click(replaceBtn);

    await waitFor(() => {
      expect(mockRefreshLessonVideos).toHaveBeenCalledWith(
        DEFAULT_PROPS.moduleId,
        DEFAULT_PROPS.sectionId,
        DEFAULT_PROPS.sectionTitle,
        VIDEO_A.videoId, // current video id passed as excludeVideoId
      );
    });

    await waitFor(() => {
      expect(screen.getByText(VIDEO_B.title)).toBeInTheDocument();
    });
  });

  it("ref_vid_replace_empty_keeps_current — Replace returning empty keeps current video", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([])); // no replacement found

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    const replaceBtn = screen.getByRole("button", { name: /replace with a different video/i });
    await user.click(replaceBtn);

    await waitFor(() => {
      expect(mockRefreshLessonVideos).toHaveBeenCalledTimes(1);
    });

    // Current video should still be shown (not blanked)
    expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
  });

  it("ref_vid_stale_replace_does_not_overwrite_new_section — Replace resolving after a section change must NOT write into the new section (WR-02)", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Stale Replace Result" };
    const VIDEO_C = { ...VIDEO_A, videoId: "vidC", title: "Service Mesh Deep Dive" };

    // sec-1 initial load resolves immediately; sec-2 load resolves immediately.
    mockGetLessonVideos
      .mockResolvedValueOnce(makeResult([VIDEO_A])) // sec-1 mount
      .mockResolvedValueOnce(makeResult([VIDEO_C])); // sec-2 mount

    // The Replace call (fired on sec-1) is DEFERRED — we resolve it manually
    // AFTER navigating to sec-2, simulating a slow in-flight request.
    let resolveReplace!: (r: LessonVideosResult) => void;
    const deferredReplace = new Promise<LessonVideosResult>((res) => {
      resolveReplace = res;
    });
    mockRefreshLessonVideos.mockReturnValueOnce(deferredReplace);

    const user = userEvent.setup();
    const { rerender } = render(
      <ReferenceVideoPanel moduleId="mod-1" sectionId="sec-1" sectionTitle="Pod Lifecycle" />,
    );

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    // Fire Replace on sec-1 (request now in-flight, not yet resolved).
    await user.click(screen.getByRole("button", { name: /replace with a different video/i }));

    // Navigate to sec-2 BEFORE the Replace resolves.
    rerender(
      <ReferenceVideoPanel moduleId="mod-1" sectionId="sec-2" sectionTitle="Service Mesh" />,
    );
    await waitFor(() => {
      expect(screen.getByText(VIDEO_C.title)).toBeInTheDocument();
    });

    // Now the stale Replace resolves with sec-1's replacement video.
    resolveReplace(makeResult([VIDEO_B]));

    // Give the microtask queue a chance to flush the stale resolution.
    await new Promise((r) => setTimeout(r, 0));

    // The new section's video must remain; the stale result must be discarded.
    expect(screen.getByText(VIDEO_C.title)).toBeInTheDocument();
    expect(screen.queryByText(VIDEO_B.title)).not.toBeInTheDocument();
  });

  // ── Back button ───────────────────────────────────────────────────────────

  it("ref_vid_back_absent_initially — Back button not visible when history is empty", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });

    expect(screen.queryByRole("button", { name: /go back to previous video/i })).toBeNull();
  });

  it("ref_vid_back_appears_after_replace — Back available after successful Replace", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Different Pod Video" };
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([VIDEO_B]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    await user.click(screen.getByRole("button", { name: /replace with a different video/i }));

    await waitFor(() => {
      expect(screen.getByText(VIDEO_B.title)).toBeInTheDocument();
    });

    // Back should now be available
    expect(screen.getByRole("button", { name: /go back to previous video/i })).toBeInTheDocument();
  });

  it("ref_vid_back_restores_previous_no_ipc — Back restores previous video instantly without IPC call", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Different Pod Video" };
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([VIDEO_B]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    // Replace current video
    await user.click(screen.getByRole("button", { name: /replace with a different video/i }));
    await waitFor(() => {
      expect(screen.getByText(VIDEO_B.title)).toBeInTheDocument();
    });

    const callCountBeforeBack = mockRefreshLessonVideos.mock.calls.length;

    // Click Back
    await user.click(screen.getByRole("button", { name: /go back to previous video/i }));

    // Should restore VIDEO_A without any new IPC calls
    expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    expect(mockRefreshLessonVideos.mock.calls.length).toBe(callCountBeforeBack);
  });

  it("ref_vid_back_absent_after_section_change — history cleared on section change", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Different Pod Video" };
    const VIDEO_C = { ...VIDEO_A, videoId: "vidC", title: "Service Mesh Deep Dive" };
    mockGetLessonVideos
      .mockResolvedValueOnce(makeResult([VIDEO_A])) // sec-1
      .mockResolvedValueOnce(makeResult([VIDEO_C])); // sec-2
    mockRefreshLessonVideos.mockResolvedValue(makeResult([VIDEO_B]));

    const user = userEvent.setup();
    const { rerender } = render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    // Replace to build up history
    await user.click(screen.getByRole("button", { name: /replace with a different video/i }));
    await waitFor(() => {
      expect(screen.getByText(VIDEO_B.title)).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /go back to previous video/i })).toBeInTheDocument();

    // Navigate to a new section
    rerender(
      <ReferenceVideoPanel moduleId="mod-1" sectionId="sec-2" sectionTitle="Service Mesh" />,
    );

    await waitFor(() => {
      expect(screen.getByText(VIDEO_C.title)).toBeInTheDocument();
    });

    // Back should be gone (history cleared)
    expect(screen.queryByRole("button", { name: /go back to previous video/i })).toBeNull();
  });

  // ── "Reference" / "optional" framing copy ────────────────────────────────

  it("ref_vid_reference_heading — renders 'Reference video' heading", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText("Reference video")).toBeInTheDocument();
    });
  });

  it("ref_vid_optional_framing — renders optional/supplementary framing copy", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      // The framing copy must mark the video as an optional extra resource.
      const panel = screen.getByTestId("reference-video-panel");
      expect(panel.textContent).toMatch(/optional/i);
      expect(panel.textContent).toMatch(/extra resource/i);
    });
  });

  it("ref_vid_full_width — panel is not width-capped (spans content width)", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      const panel = screen.getByTestId("reference-video-panel");
      expect(panel.className).toContain("w-full");
      expect(panel.className).not.toContain("max-w-lg");
    });
  });

  it("ref_vid_expand_toggle — Expand blows player to overlay; Close/Escape collapses", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-player")).toHaveAttribute(
        "data-expanded",
        "false",
      );
    });

    fireEvent.click(screen.getByLabelText("Expand reference video"));
    expect(screen.getByTestId("reference-video-player")).toHaveAttribute(
      "data-expanded",
      "true",
    );

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByTestId("reference-video-player")).toHaveAttribute(
      "data-expanded",
      "false",
    );
  });

  // ── youtube-nocookie embed (D-07) ─────────────────────────────────────────

  it("ref_vid_nocookie_embed — iframe src uses youtube-nocookie.com domain", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      const iframe = screen.getByTitle(VIDEO_A.title) as HTMLIFrameElement;
      expect(iframe.src).toContain("youtube-nocookie.com");
      expect(iframe.src).toContain(VIDEO_A.videoId);
    });
  });

  // ── Per-section re-fetch on sectionId change ───────────────────────────────

  it("ref_vid_refetch_on_section_change — calls getLessonVideos again when sectionId changes", async () => {
    const VIDEO_B = { ...VIDEO_A, videoId: "vidB", title: "Service Mesh Deep Dive" };
    mockGetLessonVideos
      .mockResolvedValueOnce(makeResult([VIDEO_A])) // sec-1
      .mockResolvedValueOnce(makeResult([VIDEO_B])); // sec-2

    const { rerender } = render(
      <ReferenceVideoPanel moduleId="mod-1" sectionId="sec-1" sectionTitle="Pod Lifecycle" />,
    );

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    // Simulate Next-lesson click — sectionId changes
    rerender(
      <ReferenceVideoPanel
        moduleId="mod-1"
        sectionId="sec-2"
        sectionTitle="Service Mesh"
      />,
    );

    await waitFor(() => {
      expect(screen.getByText(VIDEO_B.title)).toBeInTheDocument();
    });

    expect(mockGetLessonVideos).toHaveBeenCalledTimes(2);
    expect(mockGetLessonVideos).toHaveBeenNthCalledWith(1, "mod-1", "sec-1", "Pod Lifecycle");
    expect(mockGetLessonVideos).toHaveBeenNthCalledWith(2, "mod-1", "sec-2", "Service Mesh");
  });

  it("ref_vid_section_passes_correct_args — passes moduleId, sectionId, sectionTitle to IPC", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([]));

    render(
      <ReferenceVideoPanel
        moduleId="mod-xyz"
        sectionId="sec-abc"
        sectionTitle="Introduction to Pods"
      />,
    );

    await waitFor(() => {
      expect(mockGetLessonVideos).toHaveBeenCalledWith(
        "mod-xyz",
        "sec-abc",
        "Introduction to Pods",
      );
    });
  });

  // ── Refresh button ────────────────────────────────────────────────────────

  it("ref_vid_refresh_button_calls_refresh_with_section_args", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    const refreshedVideo = { ...VIDEO_A, videoId: "vidNew", title: "New Reference" };
    mockRefreshLessonVideos.mockResolvedValue(makeResult([refreshedVideo]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
    });

    const refreshBtn = screen.getByRole("button", { name: /refresh reference video/i });
    await user.click(refreshBtn);

    await waitFor(() => {
      expect(mockRefreshLessonVideos).toHaveBeenCalledWith(
        DEFAULT_PROPS.moduleId,
        DEFAULT_PROPS.sectionId,
        DEFAULT_PROPS.sectionTitle,
      );
    });

    await waitFor(() => {
      expect(screen.getByText(refreshedVideo.title)).toBeInTheDocument();
    });
  });

  it("ref_vid_refresh_null_on_empty — hides panel when refresh returns empty", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });

    const refreshBtn = screen.getByRole("button", { name: /refresh reference video/i });
    await user.click(refreshBtn);

    await waitFor(() => {
      expect(screen.queryByTestId("reference-video-panel")).toBeNull();
    });
  });

  // ── data-testid present ───────────────────────────────────────────────────

  it("ref_vid_has_testid — panel has data-testid='reference-video-panel'", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("reference-video-panel")).toBeInTheDocument();
    });
  });

  // ── Fullscreen support (D-07 + Tauri webview) ─────────────────────────────

  it("ref_vid_iframe_allow_includes_fullscreen — iframe allow list contains 'fullscreen' so native YT fullscreen works", async () => {
    mockGetLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      const iframe = screen.getByTitle(VIDEO_A.title) as HTMLIFrameElement;
      expect(iframe.getAttribute("allow")).toContain("fullscreen");
    });
  });

  // ── Empty-state generate affordance (key configured) ─────────────────────

  it("ref_vid_empty_key_configured_shows_generate_btn — when key is configured and backend returns empty, generate button is visible", async () => {
    mockIsYoutubeKeyConfigured.mockResolvedValue(true);
    mockGetLessonVideos.mockResolvedValue(makeResult([]));

    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("ref-vid-generate-btn")).toBeInTheDocument();
    });
  });

  it("ref_vid_empty_no_key_returns_null — when key is NOT configured and backend returns empty, component returns null (D-06)", async () => {
    mockIsYoutubeKeyConfigured.mockResolvedValue(false);
    mockGetLessonVideos.mockResolvedValue(makeResult([]));

    const { container } = render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(mockGetLessonVideos).toHaveBeenCalledTimes(1);
      expect(mockIsYoutubeKeyConfigured).toHaveBeenCalledTimes(1);
    });

    expect(container.firstChild).toBeNull();
    expect(screen.queryByTestId("ref-vid-generate-btn")).toBeNull();
  });

  it("ref_vid_generate_btn_click_returns_video — clicking generate button calls refreshLessonVideos and shows returned video", async () => {
    mockIsYoutubeKeyConfigured.mockResolvedValue(true);
    mockGetLessonVideos.mockResolvedValue(makeResult([]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([VIDEO_A]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("ref-vid-generate-btn")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("ref-vid-generate-btn"));

    await waitFor(() => {
      expect(mockRefreshLessonVideos).toHaveBeenCalledWith(
        DEFAULT_PROPS.moduleId,
        DEFAULT_PROPS.sectionId,
        DEFAULT_PROPS.sectionTitle,
      );
    });

    await waitFor(() => {
      expect(screen.getByText(VIDEO_A.title)).toBeInTheDocument();
      expect(screen.getByText(VIDEO_A.channelTitle)).toBeInTheDocument();
    });
  });

  it("ref_vid_generate_btn_empty_result_shows_retry — clicking generate when discovery returns empty shows 'No relevant video found' copy and retry button", async () => {
    mockIsYoutubeKeyConfigured.mockResolvedValue(true);
    mockGetLessonVideos.mockResolvedValue(makeResult([]));
    mockRefreshLessonVideos.mockResolvedValue(makeResult([]));

    const user = userEvent.setup();
    render(<ReferenceVideoPanel {...DEFAULT_PROPS} />);

    await waitFor(() => {
      expect(screen.getByTestId("ref-vid-generate-btn")).toBeInTheDocument();
    });

    await user.click(screen.getByTestId("ref-vid-generate-btn"));

    await waitFor(() => {
      // Should show the "no relevant video found" copy
      expect(screen.getByTestId("reference-video-panel")).toHaveTextContent(
        /no relevant video found/i,
      );
      // Button should still be present (now labelled "Try again")
      expect(screen.getByTestId("ref-vid-generate-btn")).toBeInTheDocument();
    });
  });
});
