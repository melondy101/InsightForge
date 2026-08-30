// MCP 工具共享的数据访问层。织知是「单一共享知识库 + 可选 Postgres」的自部署模型，
// 没有多用户体系，因此这里直接读取整库状态（loadState），由各工具在内存中过滤/检索。
// 数据量在个人知识库规模下很小，一次性读取简单且不易出错。

import { isDatabaseEnabled } from "@/lib/db/client";
import { loadState } from "@/lib/db/queries/kb";
import type { KbState } from "@/lib/api/kb";

export class KbUnavailableError extends Error {
  constructor() {
    super("database_disabled");
    this.name = "KbUnavailableError";
  }
}

/** 读取整库状态；未配置数据库时抛 KbUnavailableError，由工具转成 isError 响应。 */
export async function getKbState(): Promise<KbState> {
  if (!isDatabaseEnabled) throw new KbUnavailableError();
  return loadState();
}

/** 统一的错误响应：数据库未启用等。 */
export function kbUnavailableResult() {
  return {
    isError: true as const,
    content: [
      {
        type: "text" as const,
        text: "知识库数据库未启用（未配置 DATABASE_URL），无法通过 MCP 访问。请在部署环境配置 DATABASE_URL。",
      },
    ],
  };
}
