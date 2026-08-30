import { eq } from "drizzle-orm";
import { db, schema } from "../client";
import type { Fragment, Gap, Draft } from "@/lib/zhizhi/types";
import { DEFAULT_GUARDRAILS, DEFAULT_IDENTITY } from "@/lib/zhizhi/types";
import type { KbState } from "@/lib/api/kb";

function requireDb() {
  if (!db) throw new Error("database_disabled");
  return db;
}

export async function loadState(): Promise<KbState> {
  const d = requireDb();
  const [clusterRows, fragmentRows, gapRows, draftRows, writingRows, settingRows] =
    await Promise.all([
      d.select().from(schema.clusters),
      d.select().from(schema.fragments),
      d.select().from(schema.gaps),
      d.select().from(schema.drafts),
      d.select().from(schema.writings),
      d.select().from(schema.settings).where(eq(schema.settings.id, "default")),
    ]);

  const s = settingRows[0];
  return {
    clusters: clusterRows.map((c) => ({ id: c.id, label: c.label, fragmentIds: c.fragmentIds })),
    fragments: fragmentRows.map((f) => ({
      id: f.id,
      clusterId: f.clusterId,
      source: f.source as Fragment["source"],
      content: f.content,
      createdAt: f.createdAt.toISOString(),
      reflowed: f.reflowed,
    })),
    gaps: gapRows.map((g) => ({
      id: g.id,
      title: g.title,
      confidence: g.confidence,
      clusterIds: g.clusterIds,
      supportingFragmentIds: g.supportingFragmentIds,
      status: g.status as Gap["status"],
      draftId: g.draftId ?? undefined,
    })),
    drafts: draftRows.map((d2) => ({
      id: d2.id,
      gapId: d2.gapId,
      title: d2.title,
      content: d2.content,
      skeleton: d2.skeleton ?? undefined,
      userWords: d2.userWords,
      aiWords: d2.aiWords,
      expandUses: d2.expandUses,
      citedFragmentIds: d2.citedFragmentIds,
      updatedAt: d2.updatedAt.toISOString(),
      status: d2.status as Draft["status"],
    })),
    writings: writingRows.map((w) => ({
      id: w.id,
      draftId: w.draftId,
      gapId: w.gapId,
      title: w.title,
      content: w.content,
      userWords: w.userWords,
      aiWords: w.aiWords,
      publishedAt: w.publishedAt.toISOString(),
      reflowed: w.reflowed,
    })),
    guardrails: s
      ? {
          aiRatioLimit: s.aiRatioLimit,
          expandLimit: s.expandLimit,
          dailyInflowLimit: s.dailyInflowLimit,
        }
      : { ...DEFAULT_GUARDRAILS },
    identity: s
      ? {
          pointOfView: s.identityPov,
          audience: s.identityAudience,
          voice: s.identityVoice,
          topics: s.identityTopics,
        }
      : { ...DEFAULT_IDENTITY },
  };
}

// 全量替换保存：把前端的完整知识库状态落库（无登录、单一共享库）。
// 语义与 localStorage 一致，实现最简单、最不易出错。
export async function saveState(state: KbState): Promise<void> {
  const d = requireDb();
  await d.transaction(async (tx) => {
    await tx.delete(schema.writings);
    await tx.delete(schema.drafts);
    await tx.delete(schema.gaps);
    await tx.delete(schema.fragments);
    await tx.delete(schema.clusters);

    if (state.clusters.length) {
      await tx.insert(schema.clusters).values(
        state.clusters.map((c) => ({ id: c.id, label: c.label, fragmentIds: c.fragmentIds })),
      );
    }
    if (state.fragments.length) {
      await tx.insert(schema.fragments).values(
        state.fragments.map((f) => ({
          id: f.id,
          clusterId: f.clusterId,
          source: f.source,
          content: f.content,
          createdAt: new Date(f.createdAt),
          reflowed: Boolean(f.reflowed),
        })),
      );
    }
    if (state.gaps.length) {
      await tx.insert(schema.gaps).values(
        state.gaps.map((g) => ({
          id: g.id,
          title: g.title,
          confidence: g.confidence,
          clusterIds: g.clusterIds,
          supportingFragmentIds: g.supportingFragmentIds,
          status: g.status,
          draftId: g.draftId ?? null,
        })),
      );
    }
    if (state.drafts.length) {
      await tx.insert(schema.drafts).values(
        state.drafts.map((dr) => ({
          id: dr.id,
          gapId: dr.gapId,
          title: dr.title,
          content: dr.content,
          skeleton: dr.skeleton ?? null,
          userWords: dr.userWords,
          aiWords: dr.aiWords,
          expandUses: dr.expandUses,
          citedFragmentIds: dr.citedFragmentIds,
          updatedAt: new Date(dr.updatedAt),
          status: dr.status,
        })),
      );
    }
    if (state.writings.length) {
      await tx.insert(schema.writings).values(
        state.writings.map((w) => ({
          id: w.id,
          draftId: w.draftId,
          gapId: w.gapId,
          title: w.title,
          content: w.content,
          userWords: w.userWords,
          aiWords: w.aiWords,
          publishedAt: new Date(w.publishedAt),
          reflowed: w.reflowed,
        })),
      );
    }

    const identity = state.identity ?? DEFAULT_IDENTITY;
    await tx
      .insert(schema.settings)
      .values({
        id: "default",
        aiRatioLimit: state.guardrails.aiRatioLimit,
        expandLimit: state.guardrails.expandLimit,
        dailyInflowLimit: state.guardrails.dailyInflowLimit,
        identityPov: identity.pointOfView,
        identityAudience: identity.audience,
        identityVoice: identity.voice,
        identityTopics: identity.topics,
      })
      .onConflictDoUpdate({
        target: schema.settings.id,
        set: {
          aiRatioLimit: state.guardrails.aiRatioLimit,
          expandLimit: state.guardrails.expandLimit,
          dailyInflowLimit: state.guardrails.dailyInflowLimit,
          identityPov: identity.pointOfView,
          identityAudience: identity.audience,
          identityVoice: identity.voice,
          identityTopics: identity.topics,
        },
      });
  });
}

// 库为空时播种示例知识库
export async function isEmpty(): Promise<boolean> {
  const d = requireDb();
  const rows = await d.select({ id: schema.fragments.id }).from(schema.fragments).limit(1);
  return rows.length === 0;
}
