// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Gourav Shah (Initcron Systems Pvt. Ltd.)
//
// Phase 15 Plan 05 (D-01) — Settings section mounting RedeemLicenseFlow
// (D-02). Follows the section-per-feature shape used across Settings:
// <section> wrapper + .glass card with an icon-chip header, using the
// KeyRound icon.

import { KeyRound } from "lucide-react";
import { RedeemLicenseFlow } from "@/components/RedeemLicenseFlow";

export function SettingsRedeemLicenseSection() {
  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold text-foreground">
        Redeem license
      </h2>
      <p className="text-xs text-muted-foreground">
        Activate a licensed course pack with your license key.
      </p>

      <div className="glass rounded-xl p-5 space-y-4">
        <div className="flex items-start gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-secondary">
            <KeyRound size={20} className="text-foreground" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              License key
            </h3>
            <p className="mt-0.5 text-xs text-muted-foreground">
              Redeeming downloads and imports the licensed course pack tied
              to your key.
            </p>
          </div>
        </div>

        <RedeemLicenseFlow />
      </div>
    </section>
  );
}
