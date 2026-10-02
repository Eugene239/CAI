import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { downloadReleaseArchive, extractReleaseArchive, parseMockRunOutput } from "../dist/index.js";

test("parses the one-document result from a completed plan-only mock run", () => {
  const result = parseMockRunOutput(
    JSON.stringify({
      evidence_directory: "/tmp/cai-run-mock-run-001",
      ledger: { database: "/tmp/state.sqlite", run_id: "mock-run-001" },
      result: {
        outcome: "completed",
        execution_mode: "plan-only",
        provider: "mock",
        model: "deterministic-v1",
      },
    }),
  );

  assert.deepEqual(result, {
    outcome: "completed",
    executionMode: "plan-only",
    provider: "mock",
    model: "deterministic-v1",
    evidenceDirectory: "/tmp/cai-run-mock-run-001",
    runId: "mock-run-001",
  });
});

test("rejects output that is not a plan-only mock run", () => {
  assert.throws(
    () =>
      parseMockRunOutput(
        JSON.stringify({
          evidence_directory: "/tmp/cai-run-mock-run-001",
          ledger: { database: "/tmp/state.sqlite", run_id: "mock-run-001" },
          result: { outcome: "completed", execution_mode: "write-capable" },
        }),
      ),
    /plan-only mock run/,
  );
});

test("extracts only the CAI binary from a release archive", async () => {
  const temporaryRoot = mkdtempSync(join(tmpdir(), "cai-action-test-"));
  const sourceDirectory = join(temporaryRoot, "source");
  const archive = join(temporaryRoot, "cai.tar.gz");
  const destination = join(temporaryRoot, "destination");
  mkdirSync(sourceDirectory);
  writeFileSync(join(sourceDirectory, "cai"), "deterministic test binary");
  chmodSync(join(sourceDirectory, "cai"), 0o700);
  execFileSync("tar", ["-C", sourceDirectory, "-czf", archive, "cai"]);

  try {
    const binary = await extractReleaseArchive(readFileSync(archive), destination);
    assert.equal(readFileSync(binary, "utf8"), "deterministic test binary");
  } finally {
    rmSync(temporaryRoot, { force: true, recursive: true });
  }
});

test("follows a release-asset redirect before reading the archive", async () => {
  const server = createServer((request, response) => {
    if (request.url === "/release") {
      response.writeHead(302, { location: "/asset" });
      response.end();
      return;
    }
    response.writeHead(200, { "content-type": "application/octet-stream" });
    response.end("release archive bytes");
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const address = server.address();
  assert.notEqual(typeof address, "string");

  try {
    const archive = await downloadReleaseArchive(`http://127.0.0.1:${address.port}/release`);
    assert.equal(archive.toString("utf8"), "release archive bytes");
  } finally {
    server.close();
    await once(server, "close");
  }
});
