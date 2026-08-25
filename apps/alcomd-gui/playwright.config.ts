import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
    testDir: "./tests/browser",
    fullyParallel: false,
    retries: 0,
    timeout: 30_000,
    use: {
        baseURL: "http://127.0.0.1:4173",
        locale: "en-US",
        timezoneId: "UTC",
        trace: "retain-on-failure",
        screenshot: "only-on-failure",
        video: "off"
    },
    projects: [
        {
            name: "chromium",
            use: {
                ...devices["Desktop Chrome"],
                browserName: "chromium"
            }
        }
    ],
    webServer: {
        command: "npm run dev -- --host 127.0.0.1 --port 4173",
        url: "http://127.0.0.1:4173",
        reuseExistingServer: false,
        timeout: 30_000
    }
});
