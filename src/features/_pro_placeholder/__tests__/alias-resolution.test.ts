// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah, Vivian Aranha

import { describe, it, expect } from "vitest";
import { PluginSlot } from "@pro";

describe("@pro alias (OSS build)", () => {
  it("resolves to the no-op PluginSlot", () => {
    expect(typeof PluginSlot).toBe("function");
    const result = (PluginSlot as (p: { slotId: string }) => null)({ slotId: "x" });
    expect(result).toBeNull();
  });
});
