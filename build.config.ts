// Copyright (c) 2026 LearnForge Studio. All rights reserved.
// This file is part of LearnForge Studio and is proprietary software.
// Unauthorized copying, distribution, or modification is prohibited.
// See LICENSE-STUDIO in the repository root for terms.

import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

/** Studio overlay path constants. Read by package.json scripts and
 *  by future Vite/Tauri config files when Studio-only branching is
 *  needed. Build-time only — never imported at runtime. */
export const BUILD_CONFIG = {
  proSrc: path.resolve(__dirname, "./pro/src/features"),
  ossPlaceholder: path.resolve(__dirname, "./src/features/_pro_placeholder"),
  proTauri: path.resolve(__dirname, "./pro/src-tauri-pro"),
  isProBuild: process.env.LEARNFORGE_PRO === "1",
};
