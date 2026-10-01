import assert from "node:assert/strict";
import test from "node:test";

import { parseMockRunOutput } from "../dist/index.js";

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
