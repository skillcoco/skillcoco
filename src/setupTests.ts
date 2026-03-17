import "@testing-library/jest-dom";
import { vi } from "vitest";

// Mock @tauri-apps/api/core invoke function
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock DOM APIs not available in jsdom
Element.prototype.scrollIntoView = vi.fn();
