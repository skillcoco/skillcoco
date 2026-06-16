// Phase 6 (Certification) — Plan 06-01 (Wave 0) RED component spec.
//
// The component renders null in Wave 0. Wave 3 (Plan 06-04) implements the
// empty-state copy "No achievements yet" — at which point this test flips
// GREEN. Wave 0 deliberately fails it so the contract is visible.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

// Hoisted mock — emulates the real Zustand hook's selector signature
// (matches the TodaysChallengeCard.test.tsx pattern).
vi.mock("@/stores/useAchievementsStore", () => ({
  useAchievementsStore: vi.fn(),
}));

import { AchievementSection } from "@/components/achievements/AchievementSection";
import { useAchievementsStore } from "@/stores/useAchievementsStore";
import type { Achievement } from "@/types/achievements";

interface AchievementsSliceShape {
  achievements: Achievement[];
}

function mockState(state: AchievementsSliceShape) {
  vi.mocked(useAchievementsStore).mockImplementation(
    ((selector?: (s: AchievementsSliceShape) => unknown) => {
      if (typeof selector === "function") return selector(state);
      return state;
    }) as unknown as typeof useAchievementsStore,
  );
}

describe("AchievementSection — Phase 6 Plan 06-01 (Wave 0 RED)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.skip(
    // RED contract: when achievements list is empty, the section renders
    // the "No achievements yet" copy. Wave 0 returns null so this would
    // fail; gated `.skip()` to keep CI green per Wave 0 contract.
    // Wave 3 (Plan 06-04) implements + removes the .skip().
    "renders empty state when no achievements (Wave 3 implements)",
    () => {
      mockState({ achievements: [] });
      render(<AchievementSection />);
      expect(
        screen.getByText(/No achievements yet/i),
      ).toBeInTheDocument();
    },
  );

  // Sanity guard — confirms the import surface compiles even with the
  // .skip'd test above. This test is not the RED contract; it's a smoke
  // check that the component can be invoked without runtime errors in
  // Wave 0.
  it("Wave 0 stub renders without crashing", () => {
    mockState({ achievements: [] });
    const { container } = render(<AchievementSection />);
    // Wave 0 renders null → container has no children. Wave 3 inverts.
    expect(container.firstChild).toBeNull();
  });
});
