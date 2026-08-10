// @vitest-environment node

import {
	mkdirSync,
	mkdtempSync,
	realpathSync,
	rmSync,
	symlinkSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { postMcpHttp } from "./mcp-http.mjs";
import { canonicalizePath, isStrictChildPath } from "./path-safety.mjs";

const temporaryDirectories = [];

afterEach(() => {
	for (const directory of temporaryDirectories.splice(0)) {
		rmSync(directory, { recursive: true, force: true });
	}
});

async function withHttpServer(requestListener, callback) {
	const sockets = new Set();
	const server = createServer(requestListener);
	server.on("connection", (socket) => {
		sockets.add(socket);
		socket.once("close", () => sockets.delete(socket));
	});
	await new Promise((resolve, reject) => {
		server.once("error", reject);
		server.listen(0, "127.0.0.1", () => {
			server.off("error", reject);
			resolve();
		});
	});

	try {
		const address = server.address();
		if (!address || typeof address === "string") {
			throw new Error("HTTP test server did not expose an IP endpoint");
		}
		return await callback(address.port);
	} finally {
		for (const socket of sockets) {
			socket.destroy();
		}
		await new Promise((resolve, reject) => {
			server.close((error) => (error ? reject(error) : resolve()));
		});
	}
}

describe("MCP HTTP helper", () => {
	it("sends bearer authentication and parses JSON responses", async () => {
		await withHttpServer(
			async (request, response) => {
				let body = "";
				for await (const chunk of request) {
					body += chunk;
				}
				expect(request.url).toBe("/mcp");
				expect(request.headers.authorization).toBe("Bearer test-token");
				expect(JSON.parse(body)).toEqual({ method: "ping" });
				response.writeHead(200, {
					"content-type": "application/json",
					"mcp-session-id": "session-1",
				});
				response.end('{"ok":true}');
			},
			async (port) => {
				const result = await postMcpHttp(
					{ port, token: "test-token" },
					{ method: "ping" },
				);
				expect(result.status).toBe(200);
				expect(result.sessionId).toBe("session-1");
				expect(result.body).toEqual({ ok: true });
			},
		);
	});

	it("parses Streamable HTTP event-stream responses", async () => {
		await withHttpServer(
			(_request, response) => {
				response.writeHead(200, { "content-type": "text/event-stream" });
				response.end(
					'event: message\ndata: {"jsonrpc":"2.0","result":{"ok":true}}\n\n',
				);
			},
			async (port) => {
				const result = await postMcpHttp(
					{ port, token: "test-token" },
					{ method: "ping" },
				);
				expect(result.body.result).toEqual({ ok: true });
			},
		);
	});

	it("preserves HTTP authentication failures", async () => {
		await withHttpServer(
			(_request, response) => {
				response.writeHead(401);
				response.end("Unauthorized");
			},
			async (port) => {
				const result = await postMcpHttp(
					{ port, token: "test-token" },
					{ method: "ping" },
					{ token: null },
				);
				expect(result.status).toBe(401);
				expect(result.text).toBe("Unauthorized");
			},
		);
	});

	it("rejects an unresponsive server at the configured timeout", async () => {
		await withHttpServer(
			() => {},
			async (port) => {
				await expect(
					postMcpHttp(
						{ port, token: "test-token" },
						{ method: "ping" },
						{ timeoutMilliseconds: 25 },
					),
				).rejects.toThrow();
			},
		);
	});

	it("rejects a non-serializable request before opening a connection", async () => {
		let connections = 0;
		await withHttpServer(
			(_request, response) => {
				connections += 1;
				response.end();
			},
			async (port) => {
				const request = {};
				request.circular = request;
				await expect(
					postMcpHttp({ port, token: "test-token" }, request),
				).rejects.toThrow("circular");
			},
		);
		expect(connections).toBe(0);
	});
});

describe("desktop E2E path safety", () => {
	it("accepts strict descendants and rejects siblings", () => {
		const root = mkdtempSync(path.join(tmpdir(), "alcomd3-path-safety-"));
		temporaryDirectories.push(root);
		const allowed = path.join(root, "allowed");
		mkdirSync(allowed);

		expect(isStrictChildPath(allowed, path.join(allowed, "child"))).toBe(true);
		expect(isStrictChildPath(allowed, allowed)).toBe(false);
		expect(isStrictChildPath(allowed, path.join(root, "allowed-sibling"))).toBe(
			false,
		);
	});

	it("rejects a missing child reached through a link outside the allowed root", () => {
		const root = mkdtempSync(path.join(tmpdir(), "alcomd3-path-safety-"));
		temporaryDirectories.push(root);
		const allowed = path.join(root, "allowed");
		const outside = path.join(root, "outside");
		const link = path.join(allowed, "linked-outside");
		mkdirSync(allowed);
		mkdirSync(outside);
		symlinkSync(
			outside,
			link,
			process.platform === "win32" ? "junction" : "dir",
		);

		expect(isStrictChildPath(allowed, path.join(link, "missing"))).toBe(false);
		expect(canonicalizePath(path.join(link, "missing"))).toBe(
			path.join(realpathSync.native(outside), "missing"),
		);
	});
});
