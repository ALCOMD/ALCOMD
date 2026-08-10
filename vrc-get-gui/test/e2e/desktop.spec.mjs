import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { browser, expect } from "@wdio/globals";
import { callMcpTool, initializeLegacyMcp, postMcpHttp } from "./mcp-http.mjs";

const requiredSetupRoutes = [
	"/setup/appearance/",
	"/setup/legacy-import/",
	"/setup/unity-hub/",
	"/setup/project-path/",
	"/setup/backups/",
];

async function currentPathname() {
	const pathname = new URL(await browser.getUrl()).pathname;
	return pathname.endsWith("/") ? pathname : `${pathname}/`;
}

async function clickLastVisibleButtonAndWaitForNavigation() {
	const previousPathname = await currentPathname();
	const clicked = await browser.execute(() => {
		const visibleButtons = [...document.querySelectorAll("button")].filter(
			(button) => {
				const bounds = button.getBoundingClientRect();
				return !button.disabled && bounds.width > 0 && bounds.height > 0;
			},
		);
		const button = visibleButtons.at(-1);
		button?.click();
		return button?.textContent?.trim() ?? null;
	});
	expect(clicked).toBeTruthy();
	await browser.waitUntil(
		async () => (await currentPathname()) !== previousPathname,
		{
			timeoutMsg: `Setup did not navigate away from ${previousPathname}`,
		},
	);
}

describe("ALCOMD3 desktop startup", () => {
	it("starts the real Tauri application with an interactive first-run page", async () => {
		await browser.waitUntil(
			async () =>
				(await browser.execute(() => document.body.innerText)).trim().length >
				0,
			{
				timeoutMsg: "ALCOMD3 did not render any visible content",
			},
		);

		const page = await browser.execute(() => ({
			title: document.title,
			bodyText: document.body.innerText,
			headingCount: document.querySelectorAll("h1, h2").length,
			buttonCount: document.querySelectorAll("button").length,
		}));
		expect(page.title).toContain("ALCOMD3");
		expect(page.bodyText.toLowerCase()).not.toContain("unrecoverable error");
		expect(page.headingCount).toBeGreaterThan(0);
		expect(page.buttonCount).toBeGreaterThan(0);
		expect(await browser.getUrl()).toMatch(
			/^(?:http:\/\/tauri\.localhost|tauri:\/\/localhost)\//,
		);
		expect(await currentPathname()).toBe(requiredSetupRoutes[0]);

		const testDataRoot = process.env.ALCOMD3_TEST_LOCAL_DATA_ROOT;
		expect(testDataRoot).toBeTruthy();
		const settingsFile = path.join(testDataRoot, "ALCOMD3", "settings.json");
		expect(existsSync(settingsFile)).toBe(true);
		const settings = JSON.parse(readFileSync(settingsFile, "utf8"));
		expect(settings.userProjects).toHaveLength(1);
	});

	it("serves authenticated MCP HTTP from the GUI while data access is disabled", async () => {
		const testDataRoot = process.env.ALCOMD3_TEST_LOCAL_DATA_ROOT;
		expect(testDataRoot).toBeTruthy();
		const guiConfigFile = path.join(
			testDataRoot,
			"ALCOMD3",
			"config",
			"gui-config.json",
		);
		const guiConfig = JSON.parse(readFileSync(guiConfigFile, "utf8"));
		const endpoint = {
			port: guiConfig.mcpHttpPort,
			token: guiConfig.mcpHttpToken,
		};
		expect(endpoint.port).toBeGreaterThan(0);
		expect(endpoint.token).toMatch(/^[0-9a-f]{32}$/);

		const request = {
			jsonrpc: "2.0",
			id: "unauthorized",
			method: "initialize",
			params: {},
		};
		expect((await postMcpHttp(endpoint, request, { token: null })).status).toBe(
			401,
		);
		expect(
			(await postMcpHttp(endpoint, request, { token: "0".repeat(32) })).status,
		).toBe(401);

		const statelessMeta = {
			"io.modelcontextprotocol/protocolVersion": "2026-07-28",
			"io.modelcontextprotocol/clientInfo": {
				name: "alcomd3-desktop-e2e-stateless",
				version: "1",
			},
			"io.modelcontextprotocol/clientCapabilities": {},
		};
		const discover = await postMcpHttp(
			endpoint,
			{
				jsonrpc: "2.0",
				id: "discover",
				method: "server/discover",
				params: { _meta: statelessMeta },
			},
			{
				extraHeaders: {
					"MCP-Protocol-Version": "2026-07-28",
					"Mcp-Method": "server/discover",
				},
			},
		);
		expect(discover.status).toBe(200);
		expect(discover.sessionId).toBeNull();
		expect(discover.body.result.supportedVersions).toEqual(
			expect.arrayContaining(["2026-07-28", "2025-11-25"]),
		);

		const tools = await postMcpHttp(
			endpoint,
			{
				jsonrpc: "2.0",
				id: "tools",
				method: "tools/list",
				params: { _meta: statelessMeta },
			},
			{
				extraHeaders: {
					"MCP-Protocol-Version": "2026-07-28",
					"Mcp-Method": "tools/list",
				},
			},
		);
		expect(tools.status).toBe(200);
		expect(tools.sessionId).toBeNull();
		expect(tools.body.result.tools).toHaveLength(33);
		expect(tools.body.result.tools.map((tool) => tool.name)).toContain(
			"alcomd3_list_projects",
		);

		const sessionId = await initializeLegacyMcp(
			endpoint,
			"alcomd3-desktop-e2e",
		);
		const response = await callMcpTool(
			endpoint,
			sessionId,
			"alcomd3_list_projects",
		);
		expect(response.status).toBe(200);
		expect(JSON.stringify(response.body)).toContain("mcp_disabled");
	});

	it("completes first-run setup, discovers an isolated project, and persists setup", async () => {
		const visitedRoutes = [];
		for (let step = 0; step < 8; step += 1) {
			const pathname = await currentPathname();
			if (pathname === "/projects/") {
				break;
			}
			expect([
				...requiredSetupRoutes,
				"/setup/system-setting/",
				"/setup/finish/",
			]).toContain(pathname);
			expect(visitedRoutes).not.toContain(pathname);
			visitedRoutes.push(pathname);
			await clickLastVisibleButtonAndWaitForNavigation();
		}

		expect(await currentPathname()).toBe("/projects/");
		for (const route of requiredSetupRoutes) {
			expect(visitedRoutes).toContain(route);
		}
		expect(visitedRoutes).toContain("/setup/finish/");

		const fixtureProjectName = process.env.ALCOMD3_E2E_PROJECT_NAME;
		expect(fixtureProjectName).toBeTruthy();
		await browser.waitUntil(
			async () =>
				(await browser.execute(() => document.body.innerText)).includes(
					fixtureProjectName,
				),
			{ timeoutMsg: `Project list did not render ${fixtureProjectName}` },
		);

		const testDataRoot = process.env.ALCOMD3_TEST_LOCAL_DATA_ROOT;
		const guiConfigFile = path.join(
			testDataRoot,
			"ALCOMD3",
			"config",
			"gui-config.json",
		);
		expect(existsSync(guiConfigFile)).toBe(true);
		const guiConfig = JSON.parse(readFileSync(guiConfigFile, "utf8"));
		expect(guiConfig.mcpEnabled).toBe(false);
		expect(guiConfig.setupProcessProgress & 0x2f).toBe(0x2f);

		await browser.reloadSession();
		await browser.waitUntil(
			async () => (await currentPathname()) === "/projects/",
			{
				timeoutMsg:
					"Restarted application did not retain completed setup state",
			},
		);
		await browser.waitUntil(
			async () =>
				(await browser.execute(() => document.body.innerText)).includes(
					fixtureProjectName,
				),
			{
				timeoutMsg: `Restarted application did not retain ${fixtureProjectName}`,
			},
		);
	});

	it("runs long project operations as shared MCP Tasks with synchronous fallback", async () => {
		const testDataRoot = process.env.ALCOMD3_TEST_LOCAL_DATA_ROOT;
		const fixtureProjectName = process.env.ALCOMD3_E2E_PROJECT_NAME;
		expect(testDataRoot).toBeTruthy();
		expect(fixtureProjectName).toBeTruthy();

		const guiConfig = JSON.parse(
			readFileSync(
				path.join(testDataRoot, "ALCOMD3", "config", "gui-config.json"),
				"utf8",
			),
		);
		const endpoint = {
			port: guiConfig.mcpHttpPort,
			token: guiConfig.mcpHttpToken,
		};
		const sourceProjectPath = path.join(
			testDataRoot,
			"fixtures",
			fixtureProjectName,
		);
		const taskCopyPath = path.join(
			testDataRoot,
			"fixtures",
			"ALCOMD3 E2E Task Copy",
		);
		const syncCopyPath = path.join(
			testDataRoot,
			"fixtures",
			"ALCOMD3 E2E Sync Copy",
		);

		const enabledStatus = await browser.execute(async () => {
			return window.__TAURI_INTERNALS__.invoke("mcp_set_enabled", {
				enabled: true,
			});
		});
		expect(enabledStatus.enabled).toBe(true);

		const taskMeta = {
			"io.modelcontextprotocol/protocolVersion": "2026-07-28",
			"io.modelcontextprotocol/clientInfo": {
				name: "alcomd3-desktop-e2e-tasks",
				version: "1",
			},
			"io.modelcontextprotocol/clientCapabilities": {
				extensions: { "io.modelcontextprotocol/tasks": {} },
			},
		};
		const taskCreated = await postMcpHttp(
			endpoint,
			{
				jsonrpc: "2.0",
				id: "task-copy",
				method: "tools/call",
				params: {
					name: "alcomd3_copy_project",
					arguments: {
						source_project_path: sourceProjectPath,
						new_project_path: taskCopyPath,
					},
					_meta: taskMeta,
				},
			},
			{
				extraHeaders: {
					"MCP-Protocol-Version": "2026-07-28",
					"Mcp-Method": "tools/call",
					"Mcp-Name": "alcomd3_copy_project",
				},
				timeoutMilliseconds: 15_000,
			},
		);
		expect(taskCreated.status).toBe(200);
		expect(taskCreated.sessionId).toBeNull();
		expect(taskCreated.body.result.resultType).toBe("task");
		const taskId = taskCreated.body.result.taskId;
		expect(taskId).toBeTruthy();

		let taskResult = null;
		for (let attempt = 0; attempt < 100; attempt += 1) {
			taskResult = await postMcpHttp(
				endpoint,
				{
					jsonrpc: "2.0",
					id: `task-get-${attempt}`,
					method: "tasks/get",
					params: { taskId, _meta: taskMeta },
				},
				{
					extraHeaders: {
						"MCP-Protocol-Version": "2026-07-28",
						"Mcp-Method": "tasks/get",
						"Mcp-Name": taskId,
					},
				},
			);
			if (taskResult.body?.result?.status !== "working") {
				break;
			}
			await browser.pause(50);
		}
		expect(taskResult.status).toBe(200);
		expect(taskResult.body.result.status).toBe("completed");
		expect(existsSync(taskCopyPath)).toBe(true);

		const syncMeta = {
			...taskMeta,
			"io.modelcontextprotocol/clientCapabilities": {},
		};
		const syncResult = await postMcpHttp(
			endpoint,
			{
				jsonrpc: "2.0",
				id: "sync-copy",
				method: "tools/call",
				params: {
					name: "alcomd3_copy_project",
					arguments: {
						source_project_path: sourceProjectPath,
						new_project_path: syncCopyPath,
					},
					_meta: syncMeta,
				},
			},
			{
				extraHeaders: {
					"MCP-Protocol-Version": "2026-07-28",
					"Mcp-Method": "tools/call",
					"Mcp-Name": "alcomd3_copy_project",
				},
				timeoutMilliseconds: 15_000,
			},
		);
		expect(syncResult.status).toBe(200);
		expect(syncResult.body.result.resultType).toBe("complete");
		expect(existsSync(syncCopyPath)).toBe(true);

		const disabledStatus = await browser.execute(async () => {
			return window.__TAURI_INTERNALS__.invoke("mcp_set_enabled", {
				enabled: false,
			});
		});
		expect(disabledStatus.enabled).toBe(false);

		const getAfterDisable = await postMcpHttp(
			endpoint,
			{
				jsonrpc: "2.0",
				id: "task-get-disabled",
				method: "tasks/get",
				params: { taskId, _meta: taskMeta },
			},
			{
				extraHeaders: {
					"MCP-Protocol-Version": "2026-07-28",
					"Mcp-Method": "tasks/get",
					"Mcp-Name": taskId,
				},
			},
		);
		expect(getAfterDisable.status).toBe(200);
		expect(getAfterDisable.body.result.status).toBe("completed");

		const cancelAfterDisable = await postMcpHttp(
			endpoint,
			{
				jsonrpc: "2.0",
				id: "task-cancel-disabled",
				method: "tasks/cancel",
				params: { taskId, _meta: taskMeta },
			},
			{
				extraHeaders: {
					"MCP-Protocol-Version": "2026-07-28",
					"Mcp-Method": "tasks/cancel",
					"Mcp-Name": taskId,
				},
			},
		);
		expect(cancelAfterDisable.status).toBe(200);
		expect(cancelAfterDisable.body.result.resultType).toBe("complete");
	});
});
