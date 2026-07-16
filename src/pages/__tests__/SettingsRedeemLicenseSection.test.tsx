// Phase 15 Plan 01 (Wave 0) — SettingsRedeemLicenseSection RED scaffold test.
//
// SettingsRedeemLicenseSection doesn't exist yet (15-04 builds it) — this
// suite fails to resolve the import, which IS the RED state for this Wave 0
// plan. Mirrors the section-per-feature pattern used across Settings —
// mounts RedeemLicenseFlow (D-02) inside a `<section>` wrapper with the
// "Redeem license" title (15-UI-SPEC.md Copywriting Contract).
//
// Mocking strategy mirrors SettingsCourseImportSection.test.tsx: only
// `@/lib/tauri-commands` is mocked via vi.hoisted so the mock references
// survive vi.mock factory hoisting.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

const { redeemLicenseMock, downloadAndImportPackMock } = vi.hoisted(() => ({
  redeemLicenseMock: vi.fn(),
  downloadAndImportPackMock: vi.fn(),
}));

vi.mock("@/lib/tauri-commands", () => ({
  redeemLicense: redeemLicenseMock,
  downloadAndImportPack: downloadAndImportPackMock,
}));

// 15-04 GREEN target — does not exist yet at Wave 0 (RED: unresolved import).
import { SettingsRedeemLicenseSection } from "@/pages/SettingsRedeemLicenseSection";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("SettingsRedeemLicenseSection (Wave 0 RED)", () => {
  it("renders the 'Redeem license' section title and mounts RedeemLicenseFlow", () => {
    render(<SettingsRedeemLicenseSection />);

    expect(
      screen.getByRole("heading", { name: /redeem license/i }),
    ).toBeInTheDocument();

    // RedeemLicenseFlow's entry-stage surface (License key input + Redeem
    // button) must be mounted directly inside this section — proves D-02
    // reuse rather than a re-implementation.
    expect(screen.getByLabelText(/license key/i)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /^redeem$/i }),
    ).toBeInTheDocument();
  });
});
