import { NextResponse } from "next/server";

export function proxy() {
  return NextResponse.next();
}

export const config = {
  matcher: [
    "/dashboard/:path*",
    "/markets/:path*",
    "/events/:path*",
    "/rewards/:path*",
    "/signals/:path*",
    "/positions/:path*",
    "/risk/:path*",
    "/replay/:path*",
    "/settings/:path*",
  ],
};
