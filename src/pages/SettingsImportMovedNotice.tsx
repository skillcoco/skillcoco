// Phase 16 Plan 03 Task 3 — Settings import-moved pointer (D-03).
//
// Replaces SettingsCourseImportSection (relocated to
// src/components/library/LibraryImportSection.tsx, mounted in Library.tsx).
// No import logic, no state — a one-line pointer to the Library.

import { Link } from "react-router-dom";

export function SettingsImportMovedNotice() {
  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">Import Course</h2>
      <p className="text-sm text-muted-foreground">
        Course import has moved to the Library.{" "}
        <Link to="/library" className="font-medium text-primary hover:underline">
          Go to Library -&gt;
        </Link>
      </p>
    </section>
  );
}
