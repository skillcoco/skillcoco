# macOS Code Signing & Notarization Setup (Maintainer Playbook)

This document walks the SkillCoco release maintainer through the
one-time Apple Developer enrollment + GitHub-repo-secret population
required before pushing the first signed desktop release tag (Wave 7).

This is the **locked O-1 human-action checkpoint** from Phase 8 Plan
08-06. Claude cannot complete any step in this document — Apple
Developer enrollment is a 1-3 hour manual task plus a multi-day Apple
verification window.

---

## 1. When this applies

Run this entire playbook **once**, before pushing the first `v0.1.0`
desktop release tag. After completion the macOS-signing path in
`.github/workflows/release.yml` (Wave 3) becomes live: every subsequent
tag push automatically codesigns + notarizes + staples the `.dmg` and
`.app` artifacts via `tauri-action` and `xcrun notarytool`.

Until this playbook is completed, the 6 `APPLE_*` GitHub secrets
referenced by `release.yml` resolve to empty strings and `tauri-action`
silently skips signing. macOS users would see Gatekeeper warnings on
install — opt-in alternative documented as the "Phase 8.1 fallback"
in the plan.

---

## 2. Apple Developer Program enrollment

1. Go to https://developer.apple.com/programs/enroll/
2. Sign in with the Apple ID you intend to associate with the
   SkillCoco developer identity. Use a maintainer-controlled Apple ID,
   not a personal one — ownership transfer between Apple IDs is
   non-trivial.
3. Choose **Individual** ($99/year) or **Organization** ($299/year).
   Individual is fine for the open-core launch. Organizations require a
   D-U-N-S number and bring extra paperwork.
4. Submit. Apple verification takes **24-72 hours** in most cases, but
   can stretch to 5+ business days during high-traffic windows (post-
   WWDC, holiday seasons).
5. Wait for the "Welcome to the Apple Developer Program" email and
   confirm `Account → Membership` shows status = **Active Member**.

Total active human time: 30-60 minutes. Total elapsed time: 1-3 days
plus the multi-day Apple verification window.

---

## 3. Generate the Developer ID Application certificate

1. Open https://developer.apple.com/account → **Certificates,
   Identifiers & Profiles** → **Certificates**.
2. Click **+** → choose **Developer ID Application** (not the Mac App
   Store flavor — Developer ID is for direct distribution outside the
   App Store, which is our use case).
3. Follow the prompt to generate a Certificate Signing Request (CSR)
   in Keychain Access:
   - Open Keychain Access on macOS.
   - Menu: **Keychain Access → Certificate Assistant → Request a
     Certificate From a Certificate Authority**.
   - Email: your Apple ID email. Common Name: your name. Saved to disk.
4. Upload the `.certSigningRequest` file to Apple.
5. Download the resulting `.cer` file and double-click to import into
   Keychain Access.

---

## 4. Export the certificate as a `.p12`

1. Open Keychain Access → **login** keychain → **My Certificates**.
2. Find the "Developer ID Application: <Your Name> (<TEAM_ID>)"
   identity. Note the full string — you will need it later as
   `APPLE_SIGNING_IDENTITY`.
3. Right-click → **Export** → choose `.p12` format.
4. Set a strong export password. Save it — this is
   `APPLE_CERTIFICATE_PASSWORD`.

---

## 5. Base64-encode the `.p12` for GitHub secrets

GitHub repo secrets cannot store binary data. Convert the `.p12` to
base64 plaintext:

```bash
# macOS:
base64 -i developer_id.p12 -o developer_id.b64

# Linux (e.g. running inside a CI shell for re-encoding):
base64 -w0 developer_id.p12 > developer_id.b64
```

The contents of `developer_id.b64` is the value of the
`APPLE_CERTIFICATE` GitHub secret.

After base64 encoding, **delete the `.p12` and `.b64` files** from your
working directory. They are sensitive credentials.

---

## 6. The 6 GitHub repository secrets

`release.yml` (Wave 3) consumes exactly these 6 secrets. Populate all
of them under repo **Settings → Secrets and variables → Actions → New
repository secret** with EXACT names:

| Secret Name                  | Value Source                                                                                                                         |
|------------------------------|--------------------------------------------------------------------------------------------------------------------------------------|
| `APPLE_CERTIFICATE`          | Contents of `developer_id.b64` (base64 of `.p12`, from §5).                                                                          |
| `APPLE_CERTIFICATE_PASSWORD` | The export password you set on the `.p12` in §4.                                                                                     |
| `APPLE_SIGNING_IDENTITY`     | Full string from Keychain Access, e.g. `Developer ID Application: Jane Doe (ABCD123456)`. Must match the cert common-name exactly.   |
| `APPLE_ID`                   | The Apple ID email used for enrollment (§2).                                                                                         |
| `APPLE_PASSWORD`             | App-specific password (NOT regular Apple ID password). Generate at https://appleid.apple.com → Sign-In and Security → App-Specific Passwords. |
| `APPLE_TEAM_ID`              | 10-character team ID from https://developer.apple.com → Account → Membership (e.g. `ABCD123456`).                                    |

Critical notes:

- `APPLE_PASSWORD` is a **separate** generated password specific to
  `notarytool` — never paste your regular Apple ID password. Apple
  requires the app-specific password for any tool that signs in
  programmatically.
- `APPLE_SIGNING_IDENTITY` must be the exact common-name string of the
  certificate as Keychain reports it, parentheses and team-id included.
- `APPLE_TEAM_ID` is also embedded inside `APPLE_SIGNING_IDENTITY`
  (the parenthesized suffix). Both secrets exist so tauri-action can
  verify the binding internally.

---

## 7. Replace the `signingIdentity` placeholder in `tauri.conf.json`

`src-tauri/tauri.conf.json` ships with this placeholder inside
`bundle.macOS.signingIdentity`:

```
"signingIdentity": "Developer ID Application: <REPLACE_AFTER_ENROLLMENT> (<TEAM_ID>)"
```

After enrollment:

1. Open `src-tauri/tauri.conf.json`.
2. Replace the entire placeholder string with the same value you used
   for the `APPLE_SIGNING_IDENTITY` secret, e.g.:
   `"signingIdentity": "Developer ID Application: Jane Doe (ABCD123456)"`
3. Commit the change with message:
   `chore(08-06): replace tauri.conf.json signingIdentity placeholder post-enrollment`
4. Push to the release branch / main.

This is the single edit that wires the bundle.macOS block to the real
identity. The Wave 7 release tag will then exercise the signing path.

---

## 8. Verification

After all 6 secrets are populated and the `tauri.conf.json` placeholder
is replaced:

1. Push a release-candidate tag to a staging branch, e.g.
   `v0.1.0-rc.1`. The `release.yml` workflow runs.
2. Watch the `macos-latest` matrix entries in the workflow run. They
   should produce a notarized `.dmg` and `.app` (look for `staple`
   steps in the log).
3. Download the draft GitHub Release the workflow creates and inspect
   the macOS artifact locally:

```bash
# Validate Apple's notarization stapler is attached + Gatekeeper accepts
spctl --assess --type execute --verbose=4 path/to/SkillCoco.app

# Should print: "path/to/SkillCoco.app: accepted, source=Notarized Developer ID"
```

4. If notarization fails, check the `xcrun notarytool log <submission-id>`
   output in the workflow log. The most common failure modes are:
   - Missing `Entitlements.plist` keys (already mitigated in this repo —
     see `src-tauri/Entitlements.plist`).
   - Hardened-runtime not enabled (already `true` in tauri.conf.json).
   - App-specific password wrong (regenerate at appleid.apple.com).

5. Optionally inspect the notarytool submission history:

```bash
xcrun notarytool history \
  --apple-id "$APPLE_ID" \
  --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_PASSWORD"
```

---

## 9. Annual rotation

Apple's Developer ID certificate is valid for 5 years, but `SECURITY.md`
mandates annual rotation regardless (or immediate rotation on suspected
compromise).

Each rotation re-runs §3-§7:

1. Generate a fresh `.cer` from a new CSR.
2. Re-export `.p12`, re-base64-encode.
3. Update `APPLE_CERTIFICATE` and `APPLE_CERTIFICATE_PASSWORD` GitHub
   secrets.
4. Update `APPLE_SIGNING_IDENTITY` if Keychain reports a different
   string (rare; usually identical).
5. Re-update the `signingIdentity` field in `tauri.conf.json` if it
   changed.

Calendar a yearly reminder on `Phase 8 ship date + 11 months` so the
rotation completes before the certificate expires.

---

## 10. Troubleshooting

| Symptom                                                                | Likely Cause                                                                | Fix                                                                                       |
|------------------------------------------------------------------------|-----------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| `release.yml` succeeds but no signing step ran                         | `APPLE_SIGNING_IDENTITY` secret empty or `<REPLACE_AFTER_ENROLLMENT>` placeholder still in tauri.conf.json | Populate the secret AND replace the placeholder per §7.                                   |
| Notarization stuck for >30 minutes                                     | Apple notarization queue backed up                                          | Wait + retry. `tauri-action` has internal retry; if persistent, retry the workflow.       |
| `Hardware-signed key missing` error during codesign                    | `APPLE_CERTIFICATE` not properly base64-encoded (e.g. line-wrapped)         | Re-encode with `base64 -w0` (Linux) or `base64 -i` (macOS without wrapping).              |
| App crashes on first launch after install                              | `Entitlements.plist` missing required keys (Pitfall 2)                      | Already mitigated by `src-tauri/Entitlements.plist`. Re-verify file content if regression. |
| `spctl --assess` says "rejected"                                       | Notarization succeeded but stapler did not run                              | Check workflow logs for the `stapler staple` step; ensure tauri-action version >= 0.5.x.  |
| Apple ID rejects app-specific password                                 | Wrong password type (e.g. regular Apple ID password)                        | Regenerate at appleid.apple.com → Sign-In and Security → App-Specific Passwords.          |

---

## 11. References

- Apple Developer Program enrollment: https://developer.apple.com/programs/enroll/
- App-specific passwords: https://support.apple.com/HT204397
- Tauri 2 macOS signing docs: https://v2.tauri.app/distribute/sign/macos/
- `notarytool` reference: `man notarytool` or `xcrun notarytool --help`
- GitHub repo secrets: https://docs.github.com/en/actions/security-guides/encrypted-secrets
- SkillCoco `release.yml`: `.github/workflows/release.yml`
- SkillCoco `Entitlements.plist`: `src-tauri/Entitlements.plist`
- SkillCoco `tauri.conf.json`: `src-tauri/tauri.conf.json`

---

*Document owner: SkillCoco release maintainer*
*Originally authored as part of Phase 8 Plan 08-06 (Wave 6)*
*Required reading before pushing the first `v0.1.0` tag (Wave 7)*
