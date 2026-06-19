// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah, Vivian Aranha
//
// Phase 08.1 (Cert Split) — no-op stub for the Studio Settings Verify
// panel. The real component lives in
// `pro/src/features/components/settings/SettingsVerifyCertSection.tsx`
// and is alias-resolved via `@pro/components/settings/SettingsVerifyCertSection`
// when LEARNFORGE_PRO=1 is set on the Vite build.
//
// OSS builds resolve to this file (per `vite.config.ts` alias
// `@pro` → `src/features/_pro_placeholder`). Rendering returns `null`
// so the OSS Settings page omits the panel entirely. The backing IPC
// handlers (verify_signature, get_signing_public_key,
// fingerprint_from_public_pem, export_badge) only exist in the Studio
// Rust binary; rendering nothing here keeps `tauri-commands.ts`'s
// wrappers unreachable on the OSS path.

export function SettingsVerifyCertSection(): null {
  return null;
}
