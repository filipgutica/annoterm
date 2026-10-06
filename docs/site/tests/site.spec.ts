import { expect, test as base, type Page } from "@playwright/test";

const test = base.extend<{ cleanPage: void }>({
  cleanPage: [
    async ({ page }, use) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      page.on("console", (message) => {
        if (
          message.type() === "error" ||
          (/hydration/i.test(message.text()) && message.type() === "warning")
        )
          errors.push(message.text());
      });
      page.on("response", (response) => {
        if (
          response.url().startsWith("http://127.0.0.1:4176/") &&
          response.status() >= 400
        )
          errors.push(`${response.status()} ${response.url()}`);
      });
      await use();
      expect(errors).toEqual([]);
    },
    { auto: true },
  ],
});

const load = async (page: Page, hash = "") => {
  await page.goto(`/annoterm/${hash}`);
  await expect(page.locator(".page")).toHaveAttribute("data-enhanced", "true");
  await expect(
    page.getByRole("tab", { name: "Read", exact: true }),
  ).toBeVisible();
};
const noOverflow = async (page: Page) => {
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
};
const expectAtNativeAnchor = async (page: Page, id: string) => {
  await expect
    .poll(() =>
      page.locator(`#${id}`).evaluate((element) => {
        const inset =
          parseFloat(
            getComputedStyle(document.documentElement).scrollPaddingTop,
          ) || 0;
        const documentTop = element.getBoundingClientRect().top + scrollY;
        const maxScroll = document.documentElement.scrollHeight - innerHeight;
        const expectedTop =
          documentTop - Math.max(0, Math.min(documentTop - inset, maxScroll));
        return Math.abs(element.getBoundingClientRect().top - expectedTop);
      }),
    )
    .toBeLessThan(2);
};
const expectAtBottom = async (page: Page) => {
  await expect
    .poll(() =>
      page.evaluate(
        () => document.documentElement.scrollHeight - innerHeight - scrollY,
      ),
    )
    .toBeLessThan(2);
};
const traverseHistory = async ({
  page,
  delta,
  hash,
}: {
  page: Page;
  delta: -1 | 1;
  hash: string;
}) => {
  await page.evaluate(
    ({ delta, hash }) =>
      new Promise<void>((resolve) => {
        // The URL changes before native history scrolling has completed.
        const onScrollEnd = () => {
          if (location.hash !== hash) return;
          document.removeEventListener("scrollend", onScrollEnd);
          resolve();
        };
        document.addEventListener("scrollend", onScrollEnd);
        history.go(delta);
      }),
    { delta, hash },
  );
};

for (const width of [320, 390, 768, 801, 1280]) {
  test(`responsive layout and capture sizing at ${width}px`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    await load(page);
    await noOverflow(page);
    const frame = page.locator("#frame-read");
    await expect
      .poll(() =>
        frame
          .locator(".term")
          .evaluate(
            (element) =>
              element.getBoundingClientRect().width -
              (element.parentElement?.clientWidth ?? 0),
          ),
      )
      .toBeLessThan(2);
    await expect(frame.locator(".term")).toContainText("Retry policy");
    await expect(frame.locator(".term")).toContainText(
      "Retries use a fixed five-second delay.",
    );
    if (width <= 800)
      await page.getByRole("button", { name: "Menu", exact: true }).click();
    await expect(
      page.getByRole("navigation", { name: "Projects" }),
    ).toBeVisible();
    await expect(
      page.getByRole("navigation", { name: "On this page" }),
    ).toBeVisible();
    await expect(
      page.getByRole("link", { name: "annoterm", exact: true }).first(),
    ).toHaveAttribute("aria-current", "page");
    await expect(
      page.getByRole("link", { name: "Vue UI", exact: true }),
    ).toHaveAttribute("href", "https://filipgutica.github.io/ui/");
    await noOverflow(page);
  });
}

test("native anchor navigation, history, and scroll selection stay independent", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await load(page);
  const sectionNav = page.getByRole("navigation", { name: "On this page" });
  await sectionNav.getByRole("link", { name: "Commands", exact: true }).click();
  await expect(page).toHaveURL(/#commands-title$/);
  await expectAtNativeAnchor(page, "commands-title");
  await expect(
    sectionNav.getByRole("link", { name: "Commands", exact: true }),
  ).toHaveAttribute("aria-current", "location");
  const historyBeforeScroll = await page.evaluate(() => history.length);
  await page
    .locator("#keys-title")
    .evaluate((element) => element.scrollIntoView({ behavior: "instant" }));
  await expect(sectionNav.getByRole("link", { name: "Keys" })).toHaveAttribute(
    "aria-current",
    "location",
  );
  await expect(page).toHaveURL(/#commands-title$/);
  expect(await page.evaluate(() => history.length)).toBe(historyBeforeScroll);
  await sectionNav.getByRole("link", { name: "Good to know" }).click();
  await expect(page).toHaveURL(/#limits-title$/);
  await expectAtNativeAnchor(page, "limits-title");
  await traverseHistory({ page, delta: -1, hash: "#commands-title" });
  await expect(page).toHaveURL(/#commands-title$/);
  await traverseHistory({ page, delta: 1, hash: "#limits-title" });
  await expect(page).toHaveURL(/#limits-title$/);
  await expectAtNativeAnchor(page, "limits-title");
  await page
    .locator("#keys-title")
    .evaluate((element) => element.scrollIntoView({ behavior: "instant" }));
  await expect(sectionNav.getByRole("link", { name: "Keys" })).toHaveAttribute(
    "aria-current",
    "location",
  );
  await page.evaluate(() =>
    window.scrollTo({
      top: document.documentElement.scrollHeight,
      behavior: "instant",
    }),
  );
  await expectAtBottom(page);
  await expect(
    sectionNav.getByRole("link", { name: "Good to know" }),
  ).toHaveAttribute("aria-current", "location");
});

test("initial deep links settle after all-capture SSR becomes tabs", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await load(page, "#keys-title");
  await expectAtNativeAnchor(page, "keys-title");
  await expect(
    page
      .getByRole("navigation", { name: "On this page" })
      .getByRole("link", { name: "Keys" }),
  ).toHaveAttribute("aria-current", "location");
});

test("capture deep links and hash history reveal the requested tab", async ({
  page,
}) => {
  await load(page, "#frame-mark");
  await expect(page.getByRole("tab", { name: "Mark" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.locator("#frame-mark")).toBeVisible();
  await page.evaluate(() => {
    location.hash = "#frame-send";
  });
  await expect(
    page.getByRole("tab", { name: "Send", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await expect(page.locator("#frame-send")).toBeVisible();
  await page.goBack();
  await expect(page).toHaveURL(/#frame-mark$/);
  await expect(page.getByRole("tab", { name: "Mark" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
});

test("library tabs support keyboard navigation and retained capture content", async ({
  page,
}) => {
  await load(page);
  await page.getByRole("tab", { name: "Read", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: "Mark" })).toBeFocused();
  await expect(page.locator("#frame-mark")).toBeVisible();
  await expect(page.locator("#frame-mark .term")).toContainText(
    "Explain why this uses a",
  );
  await expect(page.locator("#frame-mark .term")).toContainText(
    "fixed delay instead of",
  );
  await page.keyboard.press("End");
  await expect(
    page.getByRole("tab", { name: "Send", exact: true }),
  ).toBeFocused();
  await expect(page.locator("#frame-send .term")).toContainText(
    "Keep annotation identifiers in your response",
  );
  await page.keyboard.press("Home");
  await expect(
    page.getByRole("tab", { name: "Read", exact: true }),
  ).toBeFocused();
});

test("full-size capture dialog keeps original terminal formatting and restores focus", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await load(page);
  const trigger = page.getByRole("button", { name: "Expand Read capture" });
  await trigger.click();
  const dialog = page.getByRole("dialog", { name: "Read capture" });
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".term")).toContainText("Retry policy");
  expect(
    await dialog
      .locator(".term")
      .evaluate((element) => getComputedStyle(element).transform),
  ).toBe("none");
  expect(
    await dialog
      .locator(".term-wrap")
      .evaluate((element) => element.scrollWidth > element.clientWidth),
  ).toBe(true);
  await noOverflow(page);
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await expect(trigger).toBeFocused();
});

test("drawer focus, dismissal, anchor destination, and desktop breakpoint", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await load(page);
  const trigger = page.getByRole("button", { name: "Menu", exact: true });
  await trigger.click();
  const drawer = page.getByRole("dialog", { name: "Navigation" });
  await expect(drawer).toBeVisible();
  for (let index = 0; index < 14; index++) await page.keyboard.press("Tab");
  expect(
    await drawer.evaluate((element) =>
      element.contains(document.activeElement),
    ),
  ).toBe(true);
  await page.keyboard.press("Escape");
  await expect(drawer).not.toBeVisible();
  await expect(trigger).toBeFocused();
  await trigger.click();
  await drawer.getByRole("link", { name: "Commands", exact: true }).click();
  await expect(drawer).not.toBeVisible();
  await expect(page.locator("#commands-title")).toBeFocused();
  await expect(page).toHaveURL(/#commands-title$/);
  await expectAtNativeAnchor(page, "commands-title");
  await trigger.click();
  await page
    .locator(".fg-drawer__overlay")
    .click({ position: { x: 380, y: 40 } });
  await expect(drawer).not.toBeVisible();
  await expect(trigger).toBeFocused();
  await trigger.click();
  await page.setViewportSize({ width: 801, height: 900 });
  await expect(drawer).not.toBeVisible();
  await expect(
    page.getByRole("navigation", { name: "Projects" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Menu", exact: true }),
  ).not.toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(trigger).toBeVisible();
  await noOverflow(page);
});

test("navigation focus survives both breakpoints without stealing content focus", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await load(page);
  const trigger = page.getByRole("button", { name: "Menu", exact: true });
  await trigger.focus();
  await page.setViewportSize({ width: 1280, height: 900 });
  await expect(
    page
      .getByRole("navigation", { name: "Projects" })
      .getByRole("link", { name: "annoterm", exact: true }),
  ).toBeFocused();
  await page
    .getByRole("navigation", { name: "On this page" })
    .getByRole("link", { name: "Commands", exact: true })
    .focus();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(trigger).toBeFocused();
  await trigger.click();
  await expect(page.getByRole("dialog", { name: "Navigation" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(trigger).toBeFocused();
  await page.setViewportSize({ width: 1280, height: 900 });
  await expect(
    page
      .getByRole("navigation", { name: "Projects" })
      .getByRole("link", { name: "annoterm", exact: true }),
  ).toBeFocused();
  const copy = page
    .locator("#install")
    .getByRole("button", { name: "Copy code" });
  await copy.focus();
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(copy).toBeFocused();
});

test("library code copying and syntax tokens work", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await load(page);
  const install = page.locator("#install");
  await install.getByRole("button", { name: "Copy code" }).click();
  await expect(install.getByRole("status")).toHaveText(
    "Code copied to clipboard.",
  );
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    "brew install filipgutica/tap/annoterm",
  );
  await expect
    .poll(() => install.locator(".fg-code-block__text > span[style]").count())
    .toBeGreaterThan(0);
});

test("clipboard failure remains recoverable with readable code", async ({
  page,
}) => {
  await page.addInitScript(() =>
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: async () => {
          throw new Error("Denied for regression check");
        },
      },
    }),
  );
  await load(page);
  const install = page.locator("#install");
  await install.getByRole("button", { name: "Copy code" }).click();
  await expect(install.getByRole("status")).toHaveText(
    "Could not copy. Select the code to copy it.",
  );
  await expect(install.locator("code")).toHaveText(
    "brew install filipgutica/tap/annoterm",
  );
  await expect(
    install.getByRole("button", { name: "Copy code" }),
  ).toBeEnabled();
});

test("themes preserve saved choice, system changes, and cross-tab storage", async ({
  page,
  context,
}) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await load(page);
  await page.getByRole("button", { name: "Dark theme", exact: true }).click();
  await expect(page.locator("html")).toHaveClass("dark");
  expect(
    await page.evaluate(() => localStorage.getItem("tool-site-theme")),
  ).toBe("dark");
  await page.reload();
  await expect(
    page.getByRole("button", { name: "Dark theme", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Light theme", exact: true }).click();
  await expect(page.locator("html")).not.toHaveClass("dark");
  await page.emulateMedia({ colorScheme: "dark" });
  await page.getByRole("button", { name: "System theme", exact: true }).click();
  await expect(page.locator("html")).toHaveClass("dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).not.toHaveClass("dark");
  const other = await context.newPage();
  await other.setViewportSize({ width: 1280, height: 900 });
  await load(other);
  await other.getByRole("button", { name: "Dark theme", exact: true }).click();
  await expect(page.locator("html")).toHaveClass("dark");
  await expect(
    page.getByRole("button", { name: "Dark theme", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await other.close();
});

test("reduced motion disables smooth anchor scrolling", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await load(page);
  expect(
    await page
      .locator("html")
      .evaluate((element) => getComputedStyle(element).scrollBehavior),
  ).toBe("auto");
  await expect(page.locator(".reveal-pending")).toHaveCount(0);
});

test("offscreen sections reveal once and recover if reduced motion changes", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await load(page);
  const commands = page.locator('section[aria-labelledby="commands-title"]');
  await expect(commands).toHaveClass(/reveal-pending/);
  await commands.scrollIntoViewIfNeeded();
  await expect(commands).not.toHaveClass(/reveal-pending/);
  await expect
    .poll(() =>
      commands.evaluate((element) => getComputedStyle(element).opacity),
    )
    .toBe("1");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(page.locator(".reveal-pending")).toHaveCount(0);
});
