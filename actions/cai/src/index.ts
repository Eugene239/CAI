import { createHash } from "node:crypto";
import { appendFile, chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";

export type MockRunOutput = {
  outcome: string;
  executionMode: "plan-only";
  provider: string;
  model: string;
  evidenceDirectory: string;
  runId: string;
};

type ActionInputs = {
  binaryUrl: string;
  binarySha256: string;
  config: string;
  repository: string;
  outputRoot: string;
  stateDatabase: string;
  runId: string;
};

type JsonObject = Record<string, unknown>;

export function parseMockRunOutput(stdout: string): MockRunOutput {
  const value = JSON.parse(stdout) as JsonObject;
  const evidenceDirectory = requiredString(value, "evidence_directory");
  const ledger = requiredObject(value, "ledger");
  const result = requiredObject(value, "result");
  const executionMode = requiredString(result, "execution_mode");

  if (executionMode !== "plan-only") {
    throw new Error("CAI output is not a plan-only mock run");
  }

  return {
    outcome: requiredString(result, "outcome"),
    executionMode,
    provider: requiredString(result, "provider"),
    model: requiredString(result, "model"),
    evidenceDirectory,
    runId: requiredString(ledger, "run_id"),
  };
}

export async function runAction(): Promise<void> {
  const inputs = readInputs();
  const workspace = await mkdtemp(join(tmpdir(), "cai-action-"));

  try {
    const archive = await download(inputs.binaryUrl);
    verifyChecksum(archive, inputs.binarySha256);
    const binaryPath = await extractReleaseArchive(archive, workspace);

    const { stdout } = await execute(
      binaryPath,
      [
        "mock",
        "run",
        "--config",
        inputs.config,
        "--repository",
        inputs.repository,
        "--output-root",
        inputs.outputRoot,
        "--state-db",
        inputs.stateDatabase,
        "--run-id",
        inputs.runId,
      ],
      "cai mock run",
    );
    const output = parseMockRunOutput(stdout);
    await publishOutputs(output);
  } finally {
    await rm(workspace, { force: true, recursive: true });
  }
}

function readInputs(): ActionInputs {
  return {
    binaryUrl: input("binary-url"),
    binarySha256: input("binary-sha256"),
    config: input("config"),
    repository: input("repository"),
    outputRoot: input("output-root"),
    stateDatabase: input("state-db"),
    runId: input("run-id"),
  };
}

function input(name: string): string {
  const value = process.env[`INPUT_${name.toUpperCase()}`] ?? process.env[`INPUT_${name.toUpperCase().replaceAll("-", "_")}`];
  if (!value?.trim()) {
    throw new Error(`missing required action input ${name}`);
  }
  return value;
}

async function download(url: string): Promise<Buffer> {
  const response = await fetch(url, { redirect: "error" });
  if (!response.ok) {
    throw new Error(`could not download CAI binary: HTTP ${response.status}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

function verifyChecksum(content: Buffer, expected: string): void {
  if (!/^[a-fA-F0-9]{64}$/.test(expected)) {
    throw new Error("binary-sha256 must be a 64-character hexadecimal SHA-256 checksum");
  }
  const actual = createHash("sha256").update(content).digest("hex");
  if (actual !== expected.toLowerCase()) {
    throw new Error("downloaded CAI binary does not match binary-sha256");
  }
}

export async function extractReleaseArchive(archive: Buffer, destination: string): Promise<string> {
  await mkdir(destination, { recursive: true });
  const archivePath = join(destination, "cai-release.tar.gz");
  await writeFile(archivePath, archive, { mode: 0o600 });

  const { stdout } = await execute("tar", ["-tzf", archivePath], "inspect CAI release archive");
  const entries = stdout.trim().split("\n").filter(Boolean);
  if (entries.length !== 1 || entries[0] !== "cai") {
    throw new Error("CAI release archive must contain exactly one cai binary");
  }

  await execute("tar", ["-xzf", archivePath, "-C", destination], "extract CAI release archive");
  const binaryPath = join(destination, "cai");
  await chmod(binaryPath, 0o700);
  return binaryPath;
}

async function execute(command: string, args: string[], description: string): Promise<{ stdout: string }> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => (stdout += chunk));
    child.stderr.on("data", (chunk: string) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(`${description} failed with exit code ${code}: ${stderr.trim()}`));
        return;
      }
      resolve({ stdout });
    });
  });
}

async function publishOutputs(output: MockRunOutput): Promise<void> {
  const target = process.env.GITHUB_OUTPUT;
  if (!target) {
    throw new Error("GITHUB_OUTPUT is not set");
  }
  const entries = [
    ["outcome", output.outcome],
    ["execution-mode", output.executionMode],
    ["provider", output.provider],
    ["model", output.model],
    ["evidence-directory", output.evidenceDirectory],
    ["run-id", output.runId],
  ];
  for (const [name, value] of entries) {
    if (value.includes("\n") || value.includes("\r")) {
      throw new Error(`output ${name} must be one line`);
    }
    await appendFile(target, `${name}=${value}\n`);
  }
}

function requiredString(object: JsonObject, key: string): string {
  const value = object[key];
  if (typeof value !== "string" || !value) {
    throw new Error(`CAI output must contain non-empty ${key}`);
  }
  return value;
}

function requiredObject(object: JsonObject, key: string): JsonObject {
  const value = object[key];
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`CAI output must contain object ${key}`);
  }
  return value as JsonObject;
}

if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  runAction().catch((error: unknown) => {
    console.error(`cai action: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  });
}
