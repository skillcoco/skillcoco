// Phase 18 (Skill Reports) — Plan 05 (Wave 3) ExportReportDialog tests.
//
// Covers: locked copy from 18-UI-SPEC.md Copywriting Contract, Dialog.Close
// aria-label, and the REP-01 "one action, two files" export click.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const exportReportPdfMock = vi.fn();
const exportReportJsonMock = vi.fn();

vi.mock("@/stores/useReportsStore", () => ({
  useReportsStore: (selector: (s: unknown) => unknown) =>
    selector({
      exportReportPdf: exportReportPdfMock,
      exportReportJson: exportReportJsonMock,
    }),
}));

import { ExportReportDialog } from "@/pages/ExportReportDialog";

describe("ExportReportDialog — Phase 18 Plan 05 (Wave 3)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    exportReportPdfMock.mockResolvedValue({ saved: true, path: "/tmp/r.pdf" });
    exportReportJsonMock.mockResolvedValue({ saved: true, path: "/tmp/r.json" });
  });

  it("renders the locked copy strings verbatim", () => {
    render(
      <ExportReportDialog
        open
        onOpenChange={vi.fn()}
        defaultScope="track"
        trackId="trk-1"
        trackTopic="Kubernetes"
        learnerName="Ada Lovelace"
      />,
    );

    expect(screen.getByText("Export skill report")).toBeInTheDocument();
    expect(screen.getByText("Your name on this report")).toBeInTheDocument();
    expect(
      screen.getByText(
        "This name is baked into the signed report. Edit it if it's out of date.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Exports as PDF (for reading) and JSON (for verification), together.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("Export report")).toBeInTheDocument();
    expect(screen.getByText("Cancel")).toBeInTheDocument();
  });

  it("aria-label='Close' is present on the dialog close control", () => {
    render(
      <ExportReportDialog
        open
        onOpenChange={vi.fn()}
        defaultScope="whole-profile"
        learnerName="Ada Lovelace"
      />,
    );
    expect(screen.getByLabelText("Close")).toBeInTheDocument();
  });

  it("pre-fills the identity input from learnerName and is editable", async () => {
    const user = userEvent.setup();
    render(
      <ExportReportDialog
        open
        onOpenChange={vi.fn()}
        defaultScope="whole-profile"
        learnerName="Ada Lovelace"
      />,
    );

    const input = screen.getByLabelText(
      "Your name on this report",
    ) as HTMLInputElement;
    expect(input.value).toBe("Ada Lovelace");

    await user.clear(input);
    await user.type(input, "Grace Hopper");
    expect(input.value).toBe("Grace Hopper");
  });

  it("clicking Export report invokes BOTH exportReportPdf and exportReportJson", async () => {
    const user = userEvent.setup();
    render(
      <ExportReportDialog
        open
        onOpenChange={vi.fn()}
        defaultScope="track"
        trackId="trk-1"
        trackTopic="Kubernetes"
        learnerName="Ada Lovelace"
      />,
    );

    await user.click(screen.getByText("Export report"));

    await waitFor(() => {
      expect(exportReportPdfMock).toHaveBeenCalledTimes(1);
      expect(exportReportJsonMock).toHaveBeenCalledTimes(1);
    });

    const pdfArgs = exportReportPdfMock.mock.calls[0][0];
    expect(pdfArgs).toMatchObject({
      scope: "track",
      trackId: "trk-1",
      trackTopic: "Kubernetes",
      learnerName: "Ada Lovelace",
    });
  });

  it("runs PDF export to completion BEFORE starting JSON export (sequential save dialogs)", async () => {
    // Each export opens a native save dialog after its IPC returns. Two
    // concurrent save() panels race on macOS (sheets serialize per window)
    // and one promise never settles — the dialog then spins forever on
    // "Exporting…". The REP-01 contract is PDF save dialog THEN JSON save
    // dialog, so the JSON export must not begin until the PDF export has
    // fully resolved.
    let resolvePdf!: (v: unknown) => void;
    exportReportPdfMock.mockReturnValue(
      new Promise((resolve) => {
        resolvePdf = resolve;
      }),
    );
    const user = userEvent.setup();
    render(
      <ExportReportDialog
        open
        onOpenChange={vi.fn()}
        defaultScope="whole-profile"
        learnerName="Ada"
      />,
    );

    await user.click(screen.getByText("Export report"));

    // PDF in flight — JSON must NOT have started yet.
    expect(exportReportPdfMock).toHaveBeenCalledTimes(1);
    expect(exportReportJsonMock).not.toHaveBeenCalled();

    resolvePdf({ saved: true, path: "/tmp/r.pdf" });

    await waitFor(() => {
      expect(exportReportJsonMock).toHaveBeenCalledTimes(1);
    });
  });

  it("shows Exporting... busy state while export is in flight", async () => {
    let resolvePdf!: (v: unknown) => void;
    exportReportPdfMock.mockReturnValue(
      new Promise((resolve) => {
        resolvePdf = resolve;
      }),
    );
    const user = userEvent.setup();
    render(
      <ExportReportDialog
        open
        onOpenChange={vi.fn()}
        defaultScope="whole-profile"
        learnerName="Ada"
      />,
    );

    await user.click(screen.getByText("Export report"));
    expect(await screen.findByText("Exporting…")).toBeInTheDocument();
    resolvePdf({ saved: true, path: "/tmp/r.pdf" });
  });

  it("defaults scope selector to whole-profile when opened without a trackId", () => {
    render(
      <ExportReportDialog
        open
        onOpenChange={vi.fn()}
        defaultScope="whole-profile"
        learnerName="Ada"
      />,
    );
    const select = screen.getByLabelText("Report scope") as HTMLSelectElement;
    expect(select.value).toBe("whole-profile");
  });
});
