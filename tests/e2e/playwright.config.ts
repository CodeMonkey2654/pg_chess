import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./studio",
  timeout: 60_000,
  use: {
    baseURL: process.env.STUDIO_UI_URL ?? "http://127.0.0.1:8081",
  },
});
