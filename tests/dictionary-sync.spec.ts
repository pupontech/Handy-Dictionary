import { test, expect, type Page } from "@playwright/test";

async function mockNative(
  page: Page,
  mode: "connected" | "connect" | "unconfigured" | "error" = "connected",
) {
  await page.addInitScript(
    ({ mode }) => {
      localStorage.setItem("i18nextLng", "en");
      let connected = mode === "connected" || mode === "error";
      let repository = connected
        ? { owner: "owner", name: "dictionary", branch: "main", id: 1 }
        : null;
      let firstSync = true;
      let lastSync: number | null = null;
      const calls: { command: string; args: unknown }[] = [];
      Object.assign(window, { nativeSyncCalls: calls });
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        value: {
          invoke: async (
            command: string,
            args: Record<string, unknown> = {},
          ) => {
            calls.push({ command, args });
            if (command === "dictionary_sync_status")
              return {
                clientId: mode === "unconfigured" ? "" : "Iv1.fixture",
                appSlug: mode === "unconfigured" ? "" : "handy-fixture",
                connected,
                repository,
                firstSync,
                lastSync,
              };
            if (command === "dictionary_sync_begin")
              return {
                userCode: "ABCD-EFGH",
                verificationUri: "https://github.com/login/device",
                interval: 5,
              };
            if (command === "dictionary_sync_poll") {
              connected = true;
              return true;
            }
            if (command === "dictionary_sync_select_repository") {
              repository = {
                owner: String(args.owner),
                name: String(args.name),
                branch: "main",
                id: 1,
              };
              return repository;
            }
            if (command === "dictionary_sync_disconnect") {
              connected = false;
              return null;
            }
            if (command === "dictionary_sync_now") {
              if (mode === "error") throw "remote_changed";
              if (!args.resolution && mode === "connect")
                return {
                  status: "conflicts",
                  conflicts: [
                    { key: "rule:heard", local: "heard → mine", remote: null },
                  ],
                };
              await new Promise((resolve) => setTimeout(resolve, 100));
              firstSync = false;
              lastSync = 1_700_000_000;
              return { status: "synced" };
            }
            return null;
          },
        },
      });
    },
    { mode },
  );
  await page.goto("/tests/fixtures/dictionary-sync.html");
  await expect(
    page.getByRole("heading", { name: "GitHub sync" }),
  ).toBeVisible();
}

test("in-app GitHub sync explains credentials and first-sync consent", async ({
  page,
}) => {
  await mockNative(page);
  await expect(page.getByText(/OS credential store/).first()).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Sync now", exact: true }),
  ).toBeDisabled();
  await page.getByRole("checkbox", { name: /first sync combines/ }).check();
  await expect(
    page.getByRole("button", { name: "Sync now", exact: true }),
  ).toBeEnabled();
});

test("connect, select private repository, resolve preview, refresh and disconnect", async ({
  page,
}) => {
  await mockNative(page, "connect");
  await page
    .getByRole("button", { name: "Connect GitHub", exact: true })
    .click();
  await expect(page.getByLabel("GitHub device code")).toHaveText("ABCD-EFGH");
  await expect(page.locator('input[type="password"]')).toHaveCount(0);
  await page
    .getByRole("button", { name: "Check authorization", exact: true })
    .click();
  await page.getByLabel("Repository owner", { exact: true }).fill("alice");
  await page
    .getByLabel("Repository name", { exact: true })
    .fill("private-dictionary");
  await page
    .getByRole("button", { name: "Use this private repository", exact: true })
    .click();
  await expect(
    page.getByText("Selected: alice/private-dictionary · branch main"),
  ).toBeVisible();
  await page.getByRole("checkbox", { name: /first sync combines/ }).check();
  await page.getByRole("button", { name: "Sync now", exact: true }).click();
  await expect(
    page.getByRole("region", {
      name: "Conflicting dictionary entries",
      exact: true,
    }),
  ).toBeVisible();
  await expect(page.getByText("This computer: heard → mine")).toBeVisible();
  await expect(page.getByText("GitHub: Deleted")).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { syncRefreshes?: number }).syncRefreshes ?? 0,
      ),
    )
    .toBe(0);
  await page
    .getByRole("button", {
      name: "Keep this computer’s conflicting entries",
      exact: true,
    })
    .click();
  await expect(
    page.getByText("Dictionary synchronized and local settings refreshed."),
  ).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as unknown as { syncRefreshes?: number }).syncRefreshes ?? 0,
      ),
    )
    .toBe(1);
  const calls = await page.evaluate(
    () =>
      (
        window as unknown as {
          nativeSyncCalls: { command: string; args: Record<string, unknown> }[];
        }
      ).nativeSyncCalls,
  );
  expect(
    calls
      .filter((c) => c.command === "dictionary_sync_now")
      .map((c) => c.args.resolution),
  ).toEqual([null, "local"]);
  expect(
    calls
      .filter((c) => c.command === "dictionary_sync_now")
      .every((c) => c.args.confirmFirstSync === true),
  ).toBeTruthy();
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Connect GitHub", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Sync now", exact: true }),
  ).toBeDisabled();
});

test("unconfigured builds do not pretend sign-in is ready", async ({
  page,
}) => {
  await mockNative(page, "unconfigured");
  await expect(
    page.getByText(/This build has no GitHub App configuration/),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Connect GitHub", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Install GitHub App", exact: true }),
  ).toBeDisabled();
});

test("errors do not refresh or claim successful sync", async ({ page }) => {
  await mockNative(page, "error");
  await page.getByRole("checkbox", { name: /first sync combines/ }).check();
  await page.getByRole("button", { name: "Sync now", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "GitHub’s dictionary changed",
  );
  expect(
    await page.evaluate(
      () =>
        (window as unknown as { syncRefreshes?: number }).syncRefreshes ?? 0,
    ),
  ).toBe(0);
  await expect(
    page.getByText("Dictionary synchronized and local settings refreshed."),
  ).toHaveCount(0);
});

test("duplicate clicks cannot start parallel native sync operations", async ({
  page,
}) => {
  await mockNative(page);
  await page.getByRole("checkbox", { name: /first sync combines/ }).check();
  await page
    .getByRole("button", { name: "Sync now", exact: true })
    .evaluate((button: HTMLButtonElement) => {
      button.click();
      button.click();
    });
  await expect(
    page.getByText("Dictionary synchronized and local settings refreshed."),
  ).toBeVisible();
  const count = await page.evaluate(
    () =>
      (
        window as unknown as { nativeSyncCalls: { command: string }[] }
      ).nativeSyncCalls.filter((c) => c.command === "dictionary_sync_now")
        .length,
  );
  expect(count).toBe(1);
});

test("full offline-readable guide distinguishes credentials, data and revocation", async ({
  page,
}) => {
  await mockNative(page);
  await page
    .getByText("Full setup, privacy, conflicts, and recovery guide (English)", {
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("heading", {
      name: "Privacy: what we do and do not store",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByRole("heading", {
      name: "Disconnecting and revoking access",
      exact: true,
    }),
  ).toBeAttached();
  await expect(
    page.getByRole("heading", {
      name: "Recovery and changing repositories",
      exact: true,
    }),
  ).toBeAttached();
  await expect(
    page.getByText("not end-to-end encrypted", { exact: true }),
  ).toBeAttached();
});
