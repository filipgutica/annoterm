/* Shared tool-site behavior. This file is identical in annoterm, wtree, and devps. */
const root = document.documentElement;
root.classList.add("js");

/* Native disclosure on phones; the full rail stays open on desktop. */
const siteMenu = document.querySelector("[data-site-menu]");
const menuToggle = siteMenu?.querySelector("summary");
if (siteMenu && menuToggle) {
  const mobileMenu = matchMedia("(max-width: 800px)");
  let menuFocus = siteMenu.contains(document.activeElement) ? document.activeElement : null;
  const syncMenu = () => {
    const focused = menuFocus;
    siteMenu.open = !mobileMenu.matches;
    if (!focused) return;
    if (mobileMenu.matches) menuToggle.focus();
    else if (focused === menuToggle) siteMenu.querySelector("a")?.focus();
  };
  syncMenu();
  mobileMenu.addEventListener("change", syncMenu);
  siteMenu.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || !mobileMenu.matches || !siteMenu.open) return;
    event.preventDefault();
    siteMenu.open = false;
    menuToggle.focus();
  });
  siteMenu.addEventListener("click", (event) => {
    if (!mobileMenu.matches || !(event.target instanceof Element)) return;
    const link = event.target.closest("a");
    if (!link) return;
    siteMenu.open = false;
    if (!link.getAttribute("href")?.startsWith("#")) return;
    const target = document.getElementById(link.hash.slice(1));
    if (!target) return;
    target.tabIndex = -1;
    target.focus({ preventScroll: true });
  });
  document.addEventListener("pointerdown", (event) => {
    if (!(event.target instanceof Node) || siteMenu.contains(event.target)) return;
    menuFocus = null;
    if (mobileMenu.matches) siteMenu.open = false;
  });
  document.addEventListener("focusin", (event) => {
    menuFocus = event.target instanceof Element && siteMenu.contains(event.target) ? event.target : null;
    if (mobileMenu.matches && !menuFocus) siteMenu.open = false;
  });
}

/* Theme: System follows the OS. The choice is remembered when storage works. */
const themeButtons = document.querySelectorAll("[data-theme-choice]");
const switcher = document.querySelector(".theme-switch");
const themeLabel = document.querySelector("[data-theme-label]");
const systemDark = matchMedia("(prefers-color-scheme: dark)");
const systemButton = document.querySelector('[data-theme-choice="system"]');
const describeSystem = () => {
  if (systemButton) {
    systemButton.title = `Follows your system: currently ${systemDark.matches ? "dark" : "light"}`;
  }
};
const setTheme = (choice) => {
  if (choice === "system") root.removeAttribute("data-theme");
  else root.dataset.theme = choice;
  themeButtons.forEach((button) =>
    button.setAttribute(
      "aria-pressed",
      String(button.dataset.themeChoice === choice),
    ),
  );
  if (switcher) switcher.dataset.choice = choice;
  if (themeLabel) themeLabel.textContent = choice[0].toUpperCase() + choice.slice(1);
};
let savedTheme = "system";
try {
  savedTheme = localStorage.getItem("tool-site-theme") || "system";
} catch {
  /* The system theme works without storage. */
}
setTheme(
  ["system", "light", "dark"].includes(savedTheme) ? savedTheme : "system",
);
describeSystem();
systemDark.addEventListener("change", describeSystem);
const appearance = document.querySelector(".appearance");
if (appearance) appearance.hidden = false;
themeButtons.forEach((button) =>
  button.addEventListener("click", () => {
    const choice = button.dataset.themeChoice;
    setTheme(choice);
    try {
      localStorage.setItem("tool-site-theme", choice);
    } catch {
      /* Keep the selected theme for this visit. */
    }
  }),
);

/* Copy: a button copies the [data-copy-text] element in its [data-copy-scope]. */
const status = document.querySelector("#copy-status");
document.querySelectorAll("[data-copy]").forEach((button) => {
  button.hidden = false;
  let reset;
  button.addEventListener("click", async () => {
    const source = button
      .closest("[data-copy-scope]")
      ?.querySelector("[data-copy-text]");
    if (!source) return;
    clearTimeout(reset);
    button.disabled = true;
    try {
      await navigator.clipboard.writeText(source.textContent.trim());
      button.textContent = "Copied";
      if (status) status.textContent = "Copied to clipboard.";
    } catch {
      button.textContent = "Copy";
      if (status)
        status.textContent =
          "Clipboard unavailable. Select the text and copy it manually.";
    } finally {
      button.disabled = false;
      reset = setTimeout(() => {
        button.textContent = "Copy";
      }, 2000);
    }
  });
});

/* Showcase: tabs step through captured frames. Without script every frame stays visible. */
document.querySelectorAll("[data-stage]").forEach((stage) => {
  const frames = [...stage.querySelectorAll(".frame")];
  const list = stage.querySelector("[data-tabs]");
  if (!list || frames.length < 2) return;
  const tabs = frames.map((frame, index) => {
    const tab = document.createElement("button");
    tab.type = "button";
    tab.role = "tab";
    tab.id = `${frame.id}-tab`;
    tab.textContent = frame.dataset.tab;
    tab.setAttribute("aria-controls", frame.id);
    frame.role = "tabpanel";
    frame.setAttribute("aria-labelledby", tab.id);
    list.append(tab);
    tab.addEventListener("click", () => select(index, false));
    tab.addEventListener("keydown", (event) => {
      const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
      if (step) {
        event.preventDefault();
        select((index + step + tabs.length) % tabs.length, true);
      } else if (event.key === "Home" || event.key === "End") {
        event.preventDefault();
        select(event.key === "Home" ? 0 : tabs.length - 1, true);
      }
    });
    return tab;
  });
  const select = (active, focus) => {
    tabs.forEach((tab, index) => {
      const on = index === active;
      tab.setAttribute("aria-selected", String(on));
      tab.tabIndex = on ? 0 : -1;
      frames[index].hidden = !on;
    });
    if (focus) tabs[active].focus();
  };
  list.setAttribute("role", "tablist");
  list.hidden = false;
  select(0, false);
});

/* Reveal offscreen documentation once. Above-the-fold content never starts hidden. */
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
if ("IntersectionObserver" in window && !reducedMotion.matches) {
  const sections = [...document.querySelectorAll(".stage, main > .split")];
  const reveal = (section) => section.classList.remove("reveal-pending");
  const observer = new IntersectionObserver((entries) => {
    entries.forEach(({ target, isIntersecting }) => {
      if (!isIntersecting) return;
      reveal(target);
      observer.unobserve(target);
    });
  });
  sections.forEach((section) => {
    if (section.getBoundingClientRect().top < innerHeight) return;
    section.classList.add("scroll-reveal", "reveal-pending");
    observer.observe(section);
  });
  reducedMotion.addEventListener("change", ({ matches }) => {
    if (!matches) return;
    sections.forEach(reveal);
    observer.disconnect();
  });
}
