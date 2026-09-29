const root = document.documentElement;
const themeButtons = document.querySelectorAll("[data-theme-choice]");
const setTheme = (choice) => {
  if (choice === "system") root.removeAttribute("data-theme");
  else root.dataset.theme = choice;
  themeButtons.forEach((button) =>
    button.setAttribute(
      "aria-pressed",
      String(button.dataset.themeChoice === choice),
    ),
  );
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
document.querySelector(".segmented").hidden = false;
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
document.querySelectorAll("[data-copy]").forEach((button) => {
  button.hidden = false;
  let reset;
  button.addEventListener("click", async () => {
    const block = button.closest(".example");
    const status = block.querySelector('[role="status"]');
    clearTimeout(reset);
    button.disabled = true;
    try {
      await navigator.clipboard.writeText(
        block.querySelector("pre code").textContent,
      );
      button.textContent = "Copied";
      status.textContent = "Command copied to clipboard.";
    } catch {
      button.textContent = "Copy";
      status.textContent =
        "Clipboard unavailable. Select the command and copy it manually.";
    } finally {
      button.disabled = false;
      reset = setTimeout(() => {
        button.textContent = "Copy";
      }, 2000);
    }
  });
});
