import { NextRequest } from "next/server";
import { isDatabaseEnabled } from "@/lib/db/client";
import { isEmpty, loadState, saveState } from "@/lib/db/queries/kb";
import type { KbState } from "@/lib/api/kb";
import {
  SEED_CLUSTERS,
  SEED_DRAFTS,
  SEED_FRAGMENTS,
  SEED_GAPS,
  SEED_WRITINGS,
} from "@/lib/zhizhi/sample-library";
import { DEFAULT_GUARDRAILS } from "@/lib/zhizhi/types";

export const dynamic = "force-dynamic";

function seedState(): KbState {
  return {
    clusters: SEED_CLUSTERS,
    fragments: SEED_FRAGMENTS,
    gaps: SEED_GAPS,
    drafts: SEED_DRAFTS,
    writings: SEED_WRITINGS,
    guardrails: { ...DEFAULT_GUARDRAILS },
  };
}

// GET /api/kb —— 读取共享知识库状态。
// 未配置数据库时返回 { enabled: false }，前端据此回退到 localStorage。
export async function GET() {
  if (!isDatabaseEnabled) {
    return Response.json({ enabled: false });
  }
  try {
    if (await isEmpty()) {
      const seed = seedState();
      await saveState(seed);
      return Response.json({ enabled: true, state: seed });
    }
    const state = await loadState();
    return Response.json({ enabled: true, state });
  } catch (error) {
    console.error("[kb] load failed", error);
    return Response.json({ enabled: false, error: "kb_load_failed" }, { status: 500 });
  }
}

// PUT /api/kb —— 全量保存知识库状态。
export async function PUT(request: NextRequest) {
  if (!isDatabaseEnabled) {
    return Response.json({ enabled: false }, { status: 409 });
  }
  const body = (await request.json().catch(() => null)) as { state?: KbState } | null;
  if (!body?.state) {
    return Response.json({ error: "missing state" }, { status: 400 });
  }
  try {
    await saveState(body.state);
    return Response.json({ ok: true });
  } catch (error) {
    console.error("[kb] save failed", error);
    return Response.json({ error: "kb_save_failed" }, { status: 500 });
  }
}
