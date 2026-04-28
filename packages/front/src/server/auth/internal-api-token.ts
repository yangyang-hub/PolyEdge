import "server-only";

import { createHash, createPrivateKey, sign as signDetached, type KeyObject } from "node:crypto";

import type { ConsoleRole } from "@/lib/console-auth";
import { getConsoleAuthMode } from "@/lib/console-auth";
import { readConsoleSession } from "@/server/auth/console-session";

export type InternalApiRequestKind = "read" | "write";

export type InternalApiStepUpScope =
  | "signal_approve"
  | "signal_reject"
  | "execution_submit"
  | "order_cancel_force"
  | "system_mode_switch"
  | "system_kill_switch_trigger"
  | "system_kill_switch_release"
  | "risk_threshold_update";

const ED25519_PKCS8_SEED_PREFIX = Buffer.from("302e020100300506032b657004220420", "hex");
const DEFAULT_ISSUER = "polyedge-nextjs";
const DEFAULT_AUDIENCE = "polyedge-rust-api";
const READ_TTL_SECS = 60;
const WRITE_TTL_SECS = 30;
const STEP_UP_WINDOW_SECS = 300;
const LOCAL_DEV_STEP_UP_CODE = "000000";

let cachedSigningKey: KeyObject | null = null;

function base64UrlEncodeJson(value: unknown): string {
  return Buffer.from(JSON.stringify(value), "utf8").toString("base64url");
}

function normalizeBase64(rawValue: string): string {
  const normalized = rawValue.trim().replace(/\s+/g, "").replace(/-/g, "+").replace(/_/g, "/");
  const remainder = normalized.length % 4;

  if (remainder === 0) {
    return normalized;
  }

  return normalized.padEnd(normalized.length + (4 - remainder), "=");
}

function roleToTokenRole(role: ConsoleRole | null): "viewer" | "operator" | "risk_admin" | "admin" {
  return role ?? "viewer";
}

function slugifyActorName(value: string | null): string {
  const normalized = (value ?? "console-user")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized || "console-user";
}

function buildSessionId(role: string, displayName: string): string {
  const digest = createHash("sha256").update(`${role}:${displayName}`).digest("hex");
  return `sess_${digest.slice(0, 20)}`;
}

function getConfiguredStepUpCode(): string | null {
  const configured = process.env.POLYEDGE_CONSOLE_STEP_UP_CODE?.trim();

  if (configured) {
    return configured;
  }

  return getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH) === "off" ? LOCAL_DEV_STEP_UP_CODE : null;
}

function verifyStepUpCode(rawCode: string | undefined, requestedScopes: InternalApiStepUpScope[]): boolean {
  if (requestedScopes.length === 0) {
    return false;
  }

  const expectedCode = getConfiguredStepUpCode();
  return Boolean(expectedCode && rawCode?.trim() === expectedCode);
}

function assertStepUpVerified(rawCode: string | undefined, requestedScopes: InternalApiStepUpScope[]): boolean {
  if (requestedScopes.length === 0) {
    return false;
  }

  if (verifyStepUpCode(rawCode, requestedScopes)) {
    return true;
  }

  throw new Error(
    "Step-up verification failed. Set POLYEDGE_CONSOLE_STEP_UP_CODE and submit the matching code for protected operations.",
  );
}

function shouldUseDevInternalAuth(): boolean {
  const kid = process.env.POLYEDGE_INTERNAL_AUTH_KID?.trim();
  const privateKey = process.env.POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY?.trim();

  if (kid || privateKey) {
    return false;
  }

  return process.env.POLYEDGE_INTERNAL_AUTH_DEV_BYPASS === "1" ||
    getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH) === "off";
}

function loadSigningKey(): { kid: string; key: KeyObject } {
  const kid = process.env.POLYEDGE_INTERNAL_AUTH_KID?.trim();
  const privateKey = process.env.POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY?.trim();

  if (!kid || !privateKey) {
    throw new Error(
      "Missing internal API auth configuration. Set POLYEDGE_INTERNAL_AUTH_KID and POLYEDGE_INTERNAL_AUTH_PRIVATE_KEY.",
    );
  }

  if (!cachedSigningKey) {
    if (privateKey.includes("BEGIN PRIVATE KEY")) {
      cachedSigningKey = createPrivateKey(privateKey);
    } else {
      const decoded = Buffer.from(normalizeBase64(privateKey), "base64");
      const derKey = decoded.length === 32 ? Buffer.concat([ED25519_PKCS8_SEED_PREFIX, decoded]) : decoded;
      cachedSigningKey = createPrivateKey({
        key: derKey,
        format: "der",
        type: "pkcs8",
      });
    }
  }

  return {
    kid,
    key: cachedSigningKey,
  };
}

async function issueInternalToken(input: {
  requestId: string;
  kind: InternalApiRequestKind;
  stepUpCode?: string;
  stepUpScopes?: InternalApiStepUpScope[];
}): Promise<string> {
  const { role, displayName } = await readConsoleSession();
  const actorRole = roleToTokenRole(role);
  const actorSlug = slugifyActorName(displayName);
  const now = Math.floor(Date.now() / 1000);
  const ttlSecs = input.kind === "write" ? WRITE_TTL_SECS : READ_TTL_SECS;
  const requestedScopes = input.stepUpScopes ?? [];
  const stepUpVerified = assertStepUpVerified(input.stepUpCode, requestedScopes);
  const { kid, key } = loadSigningKey();

  const header = {
    alg: "EdDSA",
    kid,
    typ: "JWT",
  };
  const claims = {
    iss: process.env.POLYEDGE_INTERNAL_AUTH_ISSUER?.trim() || DEFAULT_ISSUER,
    aud: process.env.POLYEDGE_INTERNAL_AUTH_AUDIENCE?.trim() || DEFAULT_AUDIENCE,
    sub: `usr_${actorSlug}`,
    iat: now,
    nbf: now,
    exp: now + ttlSecs,
    jti: `jti_${crypto.randomUUID()}`,
    session_id: buildSessionId(actorRole, actorSlug),
    roles: [actorRole],
    auth_time: now,
    request_id: input.requestId,
    step_up_verified: stepUpVerified,
    step_up_scope: stepUpVerified ? requestedScopes : [],
    step_up_until: stepUpVerified ? now + STEP_UP_WINDOW_SECS : null,
  };

  const encodedHeader = base64UrlEncodeJson(header);
  const encodedClaims = base64UrlEncodeJson(claims);
  const signingInput = `${encodedHeader}.${encodedClaims}`;
  const signature = signDetached(null, Buffer.from(signingInput, "utf8"), key).toString("base64url");

  return `${signingInput}.${signature}`;
}

export async function createInternalApiHeaders(input: {
  kind: InternalApiRequestKind;
  stepUpCode?: string;
  stepUpScopes?: InternalApiStepUpScope[];
}): Promise<Headers> {
  const requestId = `req_${crypto.randomUUID().replace(/-/g, "")}`;
  const requestedScopes = input.stepUpScopes ?? [];

  if (shouldUseDevInternalAuth()) {
    const { role, displayName } = await readConsoleSession();
    const stepUpVerified = assertStepUpVerified(input.stepUpCode, requestedScopes);

    return new Headers({
      "X-Request-Id": requestId,
      "X-PolyEdge-Dev-Auth": "local",
      "X-PolyEdge-Console-Role": roleToTokenRole(role),
      "X-PolyEdge-Console-User": encodeURIComponent(displayName ?? "Local Console"),
      "X-PolyEdge-Step-Up-Verified": String(stepUpVerified),
      "X-PolyEdge-Step-Up-Scopes": stepUpVerified ? requestedScopes.join(",") : "",
    });
  }

  const token = await issueInternalToken({
    requestId,
    kind: input.kind,
    stepUpCode: input.stepUpCode,
    stepUpScopes: input.stepUpScopes,
  });

  return new Headers({
    Authorization: `Bearer ${token}`,
    "X-Request-Id": requestId,
  });
}
