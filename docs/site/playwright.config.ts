import { defineConfig } from "@playwright/test";
import process from "node:process";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: "http://127.0.0.1:4176",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "chromium",
      testMatch: "site.spec.ts",
      use: { browserName: "chromium" },
    },
    {
      name: "chromium-no-js",
      testMatch: "no-js.spec.ts",
      use: { browserName: "chromium", javaScriptEnabled: false },
    },
  ],
  webServer: {
    command: "npm run preview -- --host 127.0.0.1 --port 4176 --ignore-lock",
    url: "http://127.0.0.1:4176/annoterm/",
    reuseExistingServer: false,
  },
});
