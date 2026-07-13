// WR-07 — single canonical getTrackColor. Previously four divergent copies
// (Sidebar, TrackView, TrackCard, LibraryPackCard); Sidebar lacked k8s/golang
// and all copies false-matched "go" as a substring ("Django", "MongoDB",
// "Google Cloud", "Algorithms" painted as Go tracks).

import { describe, it, expect } from "vitest";
import { getTrackColor } from "@/lib/track-colors";

const KUBERNETES = "hsl(var(--track-kubernetes))";
const RUST = "hsl(var(--track-rust))";
const GO = "hsl(var(--track-go))";
const PYTHON = "hsl(var(--track-python))";
const DEFAULT = "hsl(var(--primary))";

describe("getTrackColor", () => {
  it("matches kubernetes and the k8s alias", () => {
    expect(getTrackColor("Kubernetes Fundamentals")).toBe(KUBERNETES);
    expect(getTrackColor("K8s Crash Course")).toBe(KUBERNETES);
  });

  it("matches rust", () => {
    expect(getTrackColor("Rust From Zero")).toBe(RUST);
  });

  it("matches Go only as a whole word (and the golang alias)", () => {
    expect(getTrackColor("Go Basics")).toBe(GO);
    expect(getTrackColor("Learn Go")).toBe(GO);
    expect(getTrackColor("Golang for DevOps")).toBe(GO);
  });

  it("does NOT false-match 'go' inside other words", () => {
    expect(getTrackColor("Django for Beginners")).toBe(DEFAULT);
    expect(getTrackColor("MongoDB Essentials")).toBe(DEFAULT);
    expect(getTrackColor("Google Cloud Fundamentals")).toBe(DEFAULT);
    expect(getTrackColor("Algorithms 101")).toBe(DEFAULT);
  });

  it("matches python", () => {
    expect(getTrackColor("Python for DevOps")).toBe(PYTHON);
  });

  it("falls back to the primary color for unknown topics", () => {
    expect(getTrackColor("Terraform Deep Dive")).toBe(DEFAULT);
  });
});
