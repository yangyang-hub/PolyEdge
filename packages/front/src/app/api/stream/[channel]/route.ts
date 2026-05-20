import type { NextRequest } from "next/server";

import { REALTIME_CHANNELS, type RealtimeChannel } from "@/lib/contracts/realtime";
import { getApiBaseUrl } from "@/server/api/base";
import { createInternalApiHeaders } from "@/server/auth/internal-api-token";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

function isRealtimeChannel(value: string): value is RealtimeChannel {
  return REALTIME_CHANNELS.some((channel) => channel === value);
}

function createSseHeaders(extra?: HeadersInit): Headers {
  const headers = new Headers(extra);
  headers.set("Content-Type", "text/event-stream; charset=utf-8");
  headers.set("Cache-Control", "no-cache, no-transform");
  headers.set("Connection", "keep-alive");
  headers.set("X-Accel-Buffering", "no");
  return headers;
}

async function proxyLiveStream(channel: RealtimeChannel, request: NextRequest): Promise<Response> {
  const apiBaseUrl = getApiBaseUrl();

  const headers = await createInternalApiHeaders({
    kind: "read",
  });

  headers.set("Accept", "text/event-stream");
  headers.set("Cache-Control", "no-cache");
  const lastEventId = request.headers.get("last-event-id");

  if (lastEventId) {
    headers.set("Last-Event-ID", lastEventId);
  }

  const upstream = await fetch(`${apiBaseUrl}/api/v1/stream/${channel}`, {
    headers,
    cache: "no-store",
    signal: request.signal,
  });

  if (!upstream.ok || !upstream.body) {
    return new Response(`Upstream stream failed: ${upstream.status} ${upstream.statusText}`, {
      status: upstream.status || 502,
    });
  }

  return new Response(upstream.body, {
    status: upstream.status,
    headers: createSseHeaders(upstream.headers),
  });
}

export async function GET(
  request: NextRequest,
  { params }: { params: Promise<{ channel: string }> },
): Promise<Response> {
  const { channel } = await params;

  if (!isRealtimeChannel(channel)) {
    return new Response("Unknown stream channel.", { status: 404 });
  }

  return proxyLiveStream(channel, request);
}
