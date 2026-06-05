// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah, Vivian Aranha

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { PluginSlot } from "../PluginSlot";
import * as Barrel from "../index";

describe("PluginSlot (OSS no-op)", () => {
  it("renders null in OSS build", () => {
    const { container } = render(<PluginSlot slotId="test-slot" />);
    expect(container.firstChild).toBeNull();
  });

  it("accepts arbitrary context prop without throwing", () => {
    const { container } = render(
      <PluginSlot slotId="test-slot" context={{ foo: 1 }} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("exports as named export from barrel", () => {
    expect(Barrel.PluginSlot).toBe(PluginSlot);
  });
});
