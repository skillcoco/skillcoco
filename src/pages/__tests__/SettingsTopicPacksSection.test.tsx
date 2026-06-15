import { describe, it, expect } from "vitest";

/**
 * Plan 05-01 Wave 0 — RED scaffold for the SettingsTopicPacksSection
 * component (D-09). All three tests fail RED with messages naming the
 * downstream wave/plan responsible.
 *
 * Wave 3 (Plan 05-04) MUST:
 *   - Create `src/components/settings/SettingsTopicPacksSection.tsx`.
 *   - Create `src/stores/useTopicPacksStore.ts` (Zustand sibling slice).
 *   - Add `list_topic_packs`, `set_topic_pack_enabled`, `reload_skills`
 *     wrappers in `src/lib/tauri-commands.ts`.
 *   - Replace the `expect.fail` calls below with real renders + assertions.
 */
describe("SettingsTopicPacksSection (Wave 0 RED scaffold)", () => {
  it("renders pack list with id, title, source", () => {
    expect.fail(
      "Wave 3 (Plan 05-04) must create SettingsTopicPacksSection.tsx and useTopicPacksStore.ts — see plan 05-04",
    );
  });

  it("toggles a pack via setTopicPackEnabled IPC", () => {
    expect.fail(
      "Wave 3 (Plan 05-04) must wire setTopicPackEnabled IPC into the toggle handler — see plan 05-04",
    );
  });

  it("triggers reloadSkills IPC on Reload button click", () => {
    expect.fail(
      "Wave 3 (Plan 05-04) must wire reloadSkills IPC into the Reload button — see plan 05-04",
    );
  });
});
