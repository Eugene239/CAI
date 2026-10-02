import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { extractReleaseArchive, parseMockRunOutput } from "../dist/index.js";

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
