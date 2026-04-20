import type { NextRequest } from "next/server";

import { getConsoleAuthMode } from "@/lib/console-auth";
import { REALTIME_CHANNELS, type RealtimeChannel } from "@/lib/contracts/realtime";
import { readConsoleSession } from "@/server/auth/console-session";
import { getApiBaseUrl, getBackendMode } from "@/server/api/base";
import { getMockStreamEvents } from "@/server/realtime/mock-stream-events";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const encoder = new TextEncoder();

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

function formatEventMessage(event: { id: string; type: string; data: unknown }) {
  return encoder.encode(`id: ${event.id}\nevent: ${event.type}\ndata: ${JSON.stringify(event.data)}\n\n`);
}

function createMockStream(channel: RealtimeChannel, request: NextRequest): ReadableStream<Uint8Array> {
  const events = getMockStreamEvents(channel);
  const lastEventId = request.headers.get("last-event-id");
  const startIndex = Math.max(0, events.findIndex((event) => event.id === lastEventId) + 1);

  return new ReadableStream<Uint8Array>({
    start(controller) {
      let index = startIndex;
      let closed = false;

      const close = () => {
        if (closed) {
          return;
        }

        closed = true;
        clearInterval(eventTimer);
        clearInterval(heartbeatTimer);
        controller.close();
      };

      const pushEvent = () => {
        controller.enqueue(formatEventMessage(events[index % events.length]));
        index += 1;
      };

      controller.enqueue(encoder.encode(": polyedge stream ready\n\n"));
      pushEvent();

      const eventTimer = setInterval(pushEvent, 4500);
      const heartbeatTimer = setInterval(() => {
        controller.enqueue(encoder.encode(": keep-alive\n\n"));
      }, 15000);

      request.signal.addEventListener("abort", close, { once: true });
    },
  });
}

async function proxyLiveStream(channel: RealtimeChannel, request: NextRequest): Promise<Response> {
  const apiBaseUrl = getApiBaseUrl();

  if (!apiBaseUrl) {
    return new Response("Live stream base URL is not configured.", { status: 500 });
  }

  const headers = new Headers({
    Accept: "text/event-stream",
    "Cache-Control": "no-cache",
  });
  const lastEventId = request.headers.get("last-event-id");

  if (lastEventId) {
    headers.set("Last-Event-ID", lastEventId);
  }

  const upstream = await fetch(`${apiBaseUrl}/api/stream/${channel}`, {
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
  const authMode = getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH);

  if (authMode === "mock-session") {
    const session = await readConsoleSession();

    if (!session.role) {
      return new Response("Unauthorized", { status: 401 });
    }
  }

  const { channel } = await params;

  if (!isRealtimeChannel(channel)) {
    return new Response("Unknown stream channel.", { status: 404 });
  }

  if (getBackendMode() === "live") {
    return proxyLiveStream(channel, request);
  }

  return new Response(createMockStream(channel, request), {
    headers: createSseHeaders(),
  });
}
