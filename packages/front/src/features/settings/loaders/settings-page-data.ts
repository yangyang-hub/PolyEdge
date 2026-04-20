import "server-only";

import { getConsoleAuthMode } from "@/lib/console-auth";
import { getApiBaseUrl, getBackendMode } from "@/server/api/base";

export async function getSettingsPageData() {
  return {
    backendMode: getBackendMode(),
    apiBaseUrl: getApiBaseUrl(),
    consoleAuthMode: getConsoleAuthMode(process.env.POLYEDGE_CONSOLE_AUTH),
  };
}
