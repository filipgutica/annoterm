import { expect, test } from "@playwright/test";

for (const width of [320, 390, 768, 801, 1280]) {
  test(`static content and native navigation at ${width}px without JavaScript`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto("/annoterm/");
    const projectTrigger = page.getByLabel("Project: annoterm", { exact: true });
    await expect(projectTrigger).toBeVisible();
    await expect(
      page
        .getByRole("navigation", { name: "Site links" })
        .getByRole("link"),
    ).toHaveText(["Guide", "GitHub", "Releases"]);
    // The appearance picker needs JavaScript and is not rendered without it.
    await expect(
      page.getByRole("group", { name: "Color theme" }),
    ).toHaveCount(0);
    await expect(page.locator(".capture-open")).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: "Read it. Mark it. Send it back.", level: 1 }),
    ).toBeVisible();
    await expect(page.locator("#install code")).toHaveText(
      "brew install filipgutica/tap/annoterm",
    );
    await expect(page.getByRole("tab")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Copy code" })).toHaveCount(
      0,
    );
    for (const id of ["read", "mark", "send"])
      await expect(page.locator(`#frame-${id} .term`)).toBeVisible();
    await expect(page.locator("#frame-send")).toContainText(
      "Keep annotation identifiers in your response",
    );
    // The project menu is a native details element. Close it again so it does
    // not cover the local Menu on narrow screens.
    const projectMenu = page.getByRole("navigation", { name: "Projects" });
    await expect(projectMenu).toBeHidden();
    await projectTrigger.click();
    await expect(projectMenu).toBeVisible();
    await expect(projectMenu.getByRole("link")).toHaveCount(5);
    await expect(
      projectMenu.getByRole("link", { name: "UI", exact: true }),
    ).toHaveAttribute("href", "https://filipgutica.github.io/ui/");
    await expect(projectMenu.locator('[aria-current="page"]')).toHaveText(
      "annoterm",
    );
    await projectTrigger.click();
    await expect(projectMenu).toBeHidden();
    if (width <= 800) await page.locator(".site-menu > summary").click();
    const sectionNav = page.getByRole("navigation", { name: "On this page" });
    await expect(sectionNav).toBeVisible();
    await sectionNav
      .getByRole("link", { name: "Commands", exact: true })
      .click();
    await expect(page).toHaveURL(/#commands-title$/);
    await expect(page.locator("#commands-title")).toBeInViewport();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
  });
}

test("no-JavaScript deep link reaches content after every capture", async ({
  page,
}) => {
  await page.goto("/annoterm/#keys-title");
  await expect(page.locator("#keys-title")).toBeInViewport();
  await expect(page.locator("#limits-title")).toHaveText("Good to know");
});

test("no-JavaScript readers follow the OS dark theme from the library palette", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/annoterm/");
  const lightBackground = await page
    .locator("body")
    .evaluate((element) => getComputedStyle(element).backgroundColor);
  await page.emulateMedia({ colorScheme: "dark" });
  await expect
    .poll(() =>
      page
        .locator("html")
        .evaluate((element) => getComputedStyle(element).colorScheme),
    )
    .toBe("dark");
  await expect
    .poll(() =>
      page
        .locator("body")
        .evaluate((element) => getComputedStyle(element).backgroundColor),
    )
    .not.toBe(lightBackground);
  await expect(page.locator("#install code")).toHaveText(
    "brew install filipgutica/tap/annoterm",
  );
  await page.emulateMedia({ colorScheme: "light" });
  await expect
    .poll(() =>
      page
        .locator("body")
        .evaluate((element) => getComputedStyle(element).backgroundColor),
    )
    .toBe(lightBackground);
});
