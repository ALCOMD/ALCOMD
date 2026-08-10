import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
	closeSync,
	existsSync,
	mkdirSync,
	openSync,
	readdirSync,
	readFileSync,
	statSync,
	writeFileSync,
} from "node:fs";
import { createServer } from "node:net";
import path from "node:path";
import {
	callMcpTool,
	initializeLegacyMcp,
	postMcpHttp,
} from "../../vrc-get-gui/test/e2e/mcp-http.mjs";
import { isStrictChildPath } from "../../vrc-get-gui/test/e2e/path-safety.mjs";

const argumentsByName = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
	const name = process.argv[index];
	const value = process.argv[index + 1];
	if (!name?.startsWith("--") || !value) {
		throw new Error(`Invalid argument near ${name ?? "end of command"}`);
	}
	argumentsByName.set(name, value);
}

const binary = path.resolve(requiredArgument("--binary"));
const dataRoot = path.resolve(requiredArgument("--data-root"));
const runnerTempSource = process.env.RUNNER_TEMP;
const runnerTemp = runnerTempSource ? path.resolve(runnerTempSource) : null;
const label = argumentsByName.get("--label") ?? path.basename(binary);
const pidMode = argumentsByName.get("--pid-mode") ?? "exact";

if (
	process.env.GITHUB_ACTIONS !== "true" ||
	process.env.RUNNER_ENVIRONMENT !== "github-hosted" ||
	!["darwin", "linux"].includes(process.platform)
) {
	throw new Error(
		"Packaged application smoke tests may only run on an ephemeral GitHub-hosted macOS or Linux runner.",
	);
}
if (!runnerTemp || !isStrictChildPath(runnerTemp, dataRoot)) {
	throw new Error(`Data root must be a child of RUNNER_TEMP: ${dataRoot}`);
}
if (!["exact", "process-group"].includes(pidMode)) {
	throw new Error(`Unsupported --pid-mode: ${pidMode}`);
}
if (pidMode === "process-group" && process.platform !== "linux") {
	throw new Error("The process-group PID mode is only supported on Linux.");
}
if (!existsSync(binary)) {
	throw new Error(`Packaged application binary does not exist: ${binary}`);
}
if (existsSync(dataRoot) && readdirSync(dataRoot).length > 0) {
	throw new Error(
		`Packaged application data root must start empty: ${dataRoot}`,
	);
}

const home = path.join(dataRoot, "home");
const xdgDataHome = path.join(dataRoot, "xdg-data");
const xdgConfigHome = path.join(dataRoot, "xdg-config");
const xdgCacheHome = path.join(dataRoot, "xdg-cache");
const logFile = path.join(dataRoot, "application.log");
for (const directory of [home, xdgDataHome, xdgConfigHome, xdgCacheHome]) {
	mkdirSync(directory, { recursive: true });
}
const applicationDataDirectory = path.join(xdgDataHome, "ALCOMD3");
const guiConfigFile = path.join(
	applicationDataDirectory,
	"config",
	"gui-config.json",
);
mkdirSync(path.dirname(guiConfigFile), { recursive: true });
writeFileSync(
	guiConfigFile,
	`${JSON.stringify(
		{
			mcpEnabled: false,
			mcpHttpPort: await reserveLoopbackPort(),
			mcpHttpToken: randomBytes(16).toString("hex"),
		},
		null,
		4,
	)}\n`,
);

const log = openSync(logFile, "w");
const child = spawn(binary, [], {
	detached: true,
	env: {
		...process.env,
		HOME: home,
		XDG_DATA_HOME: xdgDataHome,
		XDG_CONFIG_HOME: xdgConfigHome,
		XDG_CACHE_HOME: xdgCacheHome,
		APPIMAGE_EXTRACT_AND_RUN: "1",
		WEBKIT_DISABLE_COMPOSITING_MODE: "1",
	},
	stdio: ["ignore", log, log],
});
let spawnError;
child.once("error", (error) => {
	spawnError = error;
});

let smokeError;
try {
	const endpoint = await waitForMcpHttp(
		guiConfigFile,
		child,
		() => spawnError,
		60_000,
	);
	if (
		!Number.isInteger(endpoint.port) ||
		endpoint.port < 1 ||
		endpoint.port > 65_535 ||
		!/^[0-9a-f]{32}$/.test(endpoint.token)
	) {
		throw new Error(`Invalid GUI MCP configuration: ${JSON.stringify(endpoint)}`);
	}
	assertListenerBelongsToGui(endpoint.port, child.pid, pidMode);

	const unauthenticated = await postMcpHttp(
		endpoint,
		{ jsonrpc: "2.0", id: "no-token", method: "initialize", params: {} },
		{ token: null },
	);
	if (unauthenticated.status !== 401) {
		throw new Error(
			`MCP request without a token returned ${unauthenticated.status}, expected 401.`,
		);
	}
	const unauthorized = await postMcpHttp(
		endpoint,
		{ jsonrpc: "2.0", id: "wrong-token", method: "initialize", params: {} },
		{ token: "0".repeat(32) },
	);
	if (unauthorized.status !== 401) {
		throw new Error(
			`MCP request with the wrong token returned ${unauthorized.status}, expected 401.`,
		);
	}

	const sessionId = await initializeLegacyMcp(
		endpoint,
		"alcomd3-package-smoke",
	);
	const response = await callMcpTool(
		endpoint,
		sessionId,
		"list_projects",
	);
	if (
		response.status !== 200 ||
		!JSON.stringify(response.body).includes("mcp_disabled")
	) {
		throw new Error(`Unexpected default MCP response: ${response.text}`);
	}

	if (
		!existsSync(applicationDataDirectory) ||
		!statSync(applicationDataDirectory).isDirectory()
	) {
		throw new Error(
			"The packaged application did not initialize its data directory.",
		);
	}
	console.log(`${label}: launch and local MCP boundary smoke passed.`);
} catch (error) {
	smokeError = error;
} finally {
	await terminateProcessGroup(child);
	closeSync(log);
}

if (smokeError) {
	let diagnostic = "";
	try {
		diagnostic = readFileSync(logFile, "utf8");
	} catch {}
	throw new Error(
		`${label}: packaged application smoke failed: ${smokeError.message}\nApplication log: ${logFile}\n${diagnostic}`,
		{ cause: smokeError },
	);
}

function requiredArgument(name) {
	const value = argumentsByName.get(name);
	if (!value) {
		throw new Error(`${name} is required`);
	}
	return value;
}

async function waitForMcpHttp(
	configPath,
	application,
	getSpawnError,
	timeoutMilliseconds,
) {
	const deadline = Date.now() + timeoutMilliseconds;
	while (Date.now() < deadline) {
		if (getSpawnError()) {
			throw new Error(
				`Unable to start application: ${getSpawnError().message}`,
				{
					cause: getSpawnError(),
				},
			);
		}
		if (application.exitCode !== null || application.signalCode !== null) {
			throw new Error(
				`Application exited before startup completed (exit ${application.exitCode}, signal ${application.signalCode ?? "none"}).`,
			);
		}
		if (existsSync(configPath)) {
			try {
				const config = JSON.parse(readFileSync(configPath, "utf8"));
				const endpoint = {
					port: config.mcpHttpPort,
					token: config.mcpHttpToken,
				};
				const probe = await postMcpHttp(
					endpoint,
					{ jsonrpc: "2.0", id: "probe", method: "initialize", params: {} },
					{ token: null, timeoutMilliseconds: 1_000 },
				);
				if (probe.status === 401) {
					return endpoint;
				}
			} catch {}
		}
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error(
		`Application did not start MCP HTTP from ${configPath} within ${timeoutMilliseconds / 1000} seconds.`,
	);
}

async function terminateProcessGroup(application) {
	if (!application.pid) {
		return;
	}
	try {
		process.kill(-application.pid, "SIGTERM");
	} catch (error) {
		if (error?.code !== "ESRCH") {
			throw error;
		}
	}
	await new Promise((resolve) => setTimeout(resolve, 2_000));
	if (processGroupExists(application.pid)) {
		try {
			process.kill(-application.pid, "SIGKILL");
		} catch (error) {
			if (error?.code !== "ESRCH") {
				throw error;
			}
		}
	}
}

function assertListenerBelongsToGui(port, launcherPid, mode) {
	const result = spawnSync(
		"lsof",
		["-nP", "-a", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"],
		{ encoding: "utf8" },
	);
	if (result.status !== 0) {
		throw new Error(
			`Unable to resolve the GUI process listening on MCP port ${port}: ${result.stderr}`,
		);
	}
	const listenerPids = result.stdout
		.split(/\s+/)
		.filter(Boolean)
		.map((value) => Number.parseInt(value, 10));
	const owned = listenerPids.some((listenerPid) =>
		listenerPidMatchesLaunch(listenerPid, launcherPid, mode),
	);
	if (!owned) {
		throw new Error(
			`MCP port ${port} is not owned by GUI PID ${launcherPid} or its process group; listeners: ${listenerPids.join(", ")}`,
		);
	}
}

function listenerPidMatchesLaunch(listenerPid, launcherPid, mode) {
	if (mode === "exact") {
		return listenerPid === launcherPid;
	}
	try {
		const stat = readFileSync(`/proc/${listenerPid}/stat`, "utf8");
		const fields = stat
			.slice(stat.lastIndexOf(")") + 1)
			.trim()
			.split(/\s+/);
		return Number.parseInt(fields[2], 10) === launcherPid;
	} catch {
		return false;
	}
}

function reserveLoopbackPort() {
	return new Promise((resolve, reject) => {
		const server = createServer();
		server.once("error", reject);
		server.listen(0, "127.0.0.1", () => {
			const address = server.address();
			if (!address || typeof address === "string") {
				server.close();
				reject(new Error("Unable to reserve a loopback TCP port for MCP smoke"));
				return;
			}
			server.close((error) => {
				if (error) {
					reject(error);
				} else {
					resolve(address.port);
				}
			});
		});
	});
}

function processGroupExists(processGroupId) {
	try {
		process.kill(-processGroupId, 0);
		return true;
	} catch (error) {
		if (error?.code === "ESRCH") {
			return false;
		}
		throw error;
	}
}
