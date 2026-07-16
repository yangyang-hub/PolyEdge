export function sanitizeNextPath(rawValue: string | string[] | null | undefined): string {
  const value = Array.isArray(rawValue) ? rawValue[0] : rawValue;

  if (!value || !value.startsWith("/") || value.startsWith("//")) {
    return "/dashboard";
  }

  return value;
}
