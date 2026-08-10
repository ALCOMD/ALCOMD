import { randomUUID } from "node:crypto";

const MCP_PATH = "/mcp";
const LEGACY_PROTOCOL_VERSION = "2025-11-25";

export async function postMcpHttp(
	endpoint,
	request,
	{
		token = endpoint.token,
		sessionId,
		extraHeaders = {},
		timeoutMilliseconds = 5_000,
	} = {},
) {
	const headers = {
		Accept: "application/json, text/event-stream",
		"Content-Type": "application/json",
		...extraHeaders,
	};
	if (token !== null && token !== undefined) {
		headers.Authorization = `Bearer ${token}`;
	}
	if (sessionId) {
		headers["Mcp-Session-Id"] = sessionId;
	}

	const response = await fetch(`http://127.0.0.1:${endpoint.port}${MCP_PATH}`, {
		method: "POST",
		headers,
		body: JSON.stringify(request),
		signal: AbortSignal.timeout(timeoutMilliseconds),
	});
	const text = await response.text();
	return {
		status: response.status,
		sessionId: response.headers.get("mcp-session-id"),
		body: parseMcpResponseBody(response.headers.get("content-type"), text),
		text,
	};
}

export async function initializeLegacyMcp(
	endpoint,
	clientName,
	timeoutMilliseconds = 5_000,
) {
	const requestId = randomUUID();
	const initialized = await postMcpHttp(
		endpoint,
		{
			jsonrpc: "2.0",
			id: requestId,
			method: "initialize",
			params: {
				protocolVersion: LEGACY_PROTOCOL_VERSION,
				capabilities: {},
				clientInfo: { name: clientName, version: "1" },
			},
		},
		{ timeoutMilliseconds },
	);
	if (initialized.status !== 200 || !initialized.sessionId) {
		throw new Error(
			`MCP initialize failed (${initialized.status}): ${initialized.text}`,
		);
	}

	const notification = await postMcpHttp(
		endpoint,
		{ jsonrpc: "2.0", method: "notifications/initialized" },
		{
			sessionId: initialized.sessionId,
			timeoutMilliseconds,
		},
	);
	if (![200, 202].includes(notification.status)) {
		throw new Error(
			`MCP initialized notification failed (${notification.status}): ${notification.text}`,
		);
	}
	return initialized.sessionId;
}

export async function callMcpTool(
	endpoint,
	sessionId,
	name,
	args = {},
	timeoutMilliseconds = 5_000,
) {
	const id = randomUUID();
	return postMcpHttp(
		endpoint,
		{
			jsonrpc: "2.0",
			id,
			method: "tools/call",
			params: { name, arguments: args },
		},
		{ sessionId, timeoutMilliseconds },
	);
}

function parseMcpResponseBody(contentType, text) {
	if (!text.trim()) {
		return null;
	}
	if (contentType?.includes("text/event-stream")) {
		const messages = text
			.split(/\r?\n/)
			.filter((line) => line.startsWith("data:"))
			.map((line) => line.slice(5).trim())
			.filter((line) => line && line !== "[DONE]")
			.map((line) => JSON.parse(line));
		return messages.at(-1) ?? null;
	}
	if (contentType?.includes("application/json")) {
		return JSON.parse(text);
	}
	return null;
}
