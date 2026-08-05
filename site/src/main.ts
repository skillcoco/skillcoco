import "./style.css";

const REPO = "skillcoco/skillcoco";

// ---- Theme (persisted, defaults to system preference) ----------------------
const THEME_KEY = "skillcoco-site-theme";

function applyTheme(theme: "light" | "dark") {
  document.documentElement.classList.toggle("dark", theme === "dark");
  document.querySelectorAll<HTMLElement>("[data-icon-sun]").forEach((el) => {
    el.classList.toggle("hidden", theme === "dark");
  });
  document.querySelectorAll<HTMLElement>("[data-icon-moon]").forEach((el) => {
    el.classList.toggle("hidden", theme !== "dark");
  });
}

function initialTheme(): "light" | "dark" {
  const stored = localStorage.getItem(THEME_KEY);
  if (stored === "light" || stored === "dark") return stored;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

let theme = initialTheme();
applyTheme(theme);

document.querySelectorAll<HTMLButtonElement>("[data-theme-toggle]").forEach((btn) => {
  btn.addEventListener("click", () => {
    theme = theme === "dark" ? "light" : "dark";
    localStorage.setItem(THEME_KEY, theme);
    applyTheme(theme);
  });
});

// ---- Mobile nav --------------------------------------------------------------
const mobileToggle = document.querySelector<HTMLButtonElement>("[data-mobile-nav-toggle]");
const mobileMenu = document.querySelector<HTMLElement>("[data-mobile-nav]");
mobileToggle?.addEventListener("click", () => {
  const isOpen = mobileMenu?.classList.toggle("flex");
  mobileMenu?.classList.toggle("hidden");
  mobileToggle.setAttribute("aria-expanded", String(Boolean(isOpen)));
});
document.querySelectorAll<HTMLAnchorElement>("[data-mobile-nav] a").forEach((a) => {
  a.addEventListener("click", () => {
    mobileMenu?.classList.add("hidden");
    mobileMenu?.classList.remove("flex");
    mobileToggle?.setAttribute("aria-expanded", "false");
  });
});

// ---- Reveal-on-scroll ---------------------------------------------------------
const revealEls = document.querySelectorAll<HTMLElement>(".reveal");
if ("IntersectionObserver" in window && revealEls.length) {
  const io = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          io.unobserve(entry.target);
        }
      }
    },
    { threshold: 0.1, rootMargin: "0px 0px -40px 0px" },
  );
  revealEls.forEach((el) => io.observe(el));

  // Safety net: below-the-fold sections must never stay invisible forever.
  // Full-page screenshot/print tools (and CDP screenshot capture in general)
  // often expand the render surface without dispatching real scroll events,
  // so IntersectionObserver never fires for elements outside the true
  // viewport. Force everything visible after a short delay regardless.
  setTimeout(() => {
    revealEls.forEach((el) => el.classList.add("is-visible"));
    io.disconnect();
  }, 1200);
} else {
  revealEls.forEach((el) => el.classList.add("is-visible"));
}

// ---- Footer year --------------------------------------------------------------
document.querySelectorAll<HTMLElement>("[data-year]").forEach((el) => {
  el.textContent = String(new Date().getFullYear());
});

// ---- Copy install command -------------------------------------------------------
document.querySelectorAll<HTMLButtonElement>("[data-copy]").forEach((btn) => {
  const targetSel = btn.getAttribute("data-copy");
  if (!targetSel) return;
  const source = document.querySelector<HTMLElement>(targetSel);
  btn.addEventListener("click", async () => {
    const text = source?.textContent?.trim() ?? "";
    try {
      await navigator.clipboard.writeText(text);
      const original = btn.textContent;
      btn.textContent = "Copied!";
      setTimeout(() => {
        btn.textContent = original;
      }, 1500);
    } catch {
      // Clipboard API unavailable — fail silently, the command is selectable text.
    }
  });
});

// ---- GitHub live data: stars + latest release ------------------------------------
async function loadGithubStats() {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}`, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return;
    const data = await res.json();
    const stars = data.stargazers_count;
    if (typeof stars === "number") {
      document.querySelectorAll<HTMLElement>("[data-github-stars]").forEach((el) => {
        el.textContent = stars >= 1000 ? `${(stars / 1000).toFixed(1)}k` : String(stars);
      });
    }
  } catch {
    // Network unavailable / rate-limited — the static "Star on GitHub" CTA still works.
  }
}

async function loadLatestRelease() {
  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return;
    const data = await res.json();
    const tag: string | undefined = data.tag_name;
    const dmgAsset = (data.assets ?? []).find((a: { name: string }) => a.name.endsWith(".dmg"));
    if (tag) {
      document.querySelectorAll<HTMLElement>("[data-latest-version]").forEach((el) => {
        el.textContent = tag;
      });
    }
    if (dmgAsset?.browser_download_url) {
      document.querySelectorAll<HTMLAnchorElement>("[data-download-dmg]").forEach((el) => {
        el.href = dmgAsset.browser_download_url;
      });
    }
  } catch {
    // Fall back to the static /releases/latest link already in the markup.
  }
}

loadGithubStats();
loadLatestRelease();
