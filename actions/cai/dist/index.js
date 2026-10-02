import { createHash } from "node:crypto";
import { appendFile, chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
export function parseMockRunOutput(stdout) {
    const value = JSON.parse(stdout);
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
export async function runAction() {
    const inputs = readInputs();
    const workspace = await mkdtemp(join(tmpdir(), "cai-action-"));
    const binaryPath = join(workspace, "cai");
    try {
        const archive = await download(inputs.binaryUrl);
        verifyChecksum(archive, inputs.binarySha256);
        await writeFile(binaryPath, archive, { mode: 0o700 });
        await chmod(binaryPath, 0o700);
        const { stdout } = await execute(binaryPath, [
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
        ]);
        const output = parseMockRunOutput(stdout);
        await publishOutputs(output);
    }
    finally {
        await rm(workspace, { force: true, recursive: true });
    }
}
function readInputs() {
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
function input(name) {
    const value = process.env[`INPUT_${name.toUpperCase()}`] ?? process.env[`INPUT_${name.toUpperCase().replaceAll("-", "_")}`];
    if (!value?.trim()) {
        throw new Error(`missing required action input ${name}`);
    }
    return value;
}
async function download(url) {
    const response = await fetch(url, { redirect: "error" });
    if (!response.ok) {
        throw new Error(`could not download CAI binary: HTTP ${response.status}`);
    }
    return Buffer.from(await response.arrayBuffer());
}
function verifyChecksum(content, expected) {
    if (!/^[a-fA-F0-9]{64}$/.test(expected)) {
        throw new Error("binary-sha256 must be a 64-character hexadecimal SHA-256 checksum");
    }
    const actual = createHash("sha256").update(content).digest("hex");
    if (actual !== expected.toLowerCase()) {
        throw new Error("downloaded CAI binary does not match binary-sha256");
    }
}
async function execute(command, args) {
    return new Promise((resolve, reject) => {
        const child = spawn(command, args, { stdio: ["ignore", "pipe", "pipe"] });
        let stdout = "";
        let stderr = "";
        child.stdout.setEncoding("utf8");
        child.stderr.setEncoding("utf8");
        child.stdout.on("data", (chunk) => (stdout += chunk));
        child.stderr.on("data", (chunk) => (stderr += chunk));
        child.on("error", reject);
        child.on("close", (code) => {
            if (code !== 0) {
                reject(new Error(`cai mock run failed with exit code ${code}: ${stderr.trim()}`));
                return;
            }
            resolve({ stdout });
        });
    });
}
async function publishOutputs(output) {
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
function requiredString(object, key) {
    const value = object[key];
    if (typeof value !== "string" || !value) {
        throw new Error(`CAI output must contain non-empty ${key}`);
    }
    return value;
}
function requiredObject(object, key) {
    const value = object[key];
    if (!value || typeof value !== "object" || Array.isArray(value)) {
        throw new Error(`CAI output must contain object ${key}`);
    }
    return value;
}
if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
    runAction().catch((error) => {
        console.error(`cai action: ${error instanceof Error ? error.message : String(error)}`);
        process.exitCode = 1;
    });
}
