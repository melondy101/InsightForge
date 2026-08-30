# InsightForge 首版平台导出方案

> 调研日期：2026-08-30  
> 范围：Markdown 草稿输出为适合外部发布平台的格式；不改变知识库输入格式与本地优先边界。

## 结论

Markdown 是唯一的草稿真相。首版采用可扩展的“平台导出器”，只实现两个目标：

1. `generic-html`：从 CommonMark + GFM 生成语义化、已消毒、包含阅读排版样式的完整 HTML 文件，供 WordPress 与其他 CMS 粘贴或导入。
2. `wechat-clipboard-html`：在同一 Markdown AST 基础上生成标签/属性受限、样式内联的 HTML 片段，并同时写入 `text/html` 与 `text/plain` 剪贴板，供用户手动粘贴到微信公众号编辑器。

不做微信 API 直发、草稿箱同步、图片托管或复杂富媒体。它们会引入平台账号密钥、素材上传、发布权限与额外安全责任，且违背应用止于“帮助形成草稿”的边界。

## 边界

- 首版知识库输入仍只有 `.md` 与 `.txt`。
- 平台输出是 Markdown 的派生物，不反向修改草稿。
- 图片只保留远程 `https` URL；本地图片提示用户先上传或替换 URL。不要首版 base64 内嵌。
- 通用 HTML 保留安全链接；公众号输出同样保留安全链接，不安全协议降级为纯文本。
- 不为 WordPress 单独建立 Markdown 方言。其 Posts REST API 的 `content` 字段可以接收 HTML，未来若增加“推送草稿”，在通用 HTML 之上增加传输适配器即可。

## 实现建议

导出层放在 React/TypeScript 一侧。`remark-parse → remark-gfm → remark-rehype → rehype-stringify` 的 Markdown AST 到 HTML AST 管线，适合共享解析、消毒和目标平台变换；官方用法也要求输出 HTML 时加入 sanitize。不要把平台格式化逻辑塞入编辑器或 Rust 文件服务。

```ts
type ExportTarget = "generic-html" | "wechat-clipboard-html";

interface PlatformAdapter {
  id: ExportTarget;
  label: string;
  render(input: MarkdownDocument): ExportArtifact;
  capabilities: {
    images: "remote-only" | "embed" | "placeholder";
    links: "preserve" | "footnotes";
    tables: boolean;
    math: "preserve" | "fallback";
  };
}
```

微信公众号适配需要独立的标签白名单、属性清理和内联样式转换；它不只是换一个 CSS 主题。可以借鉴 `doocs/md` 与 `PostPress` 的转换管线和兼容性测试，但不把 PostPress 作为核心依赖：它规模较小，并带有 `sharp`、Playwright、Mermaid 等超出 MVP 的运行时。

## 排版优化

采用“一个 Markdown 真相、两套输出皮肤”：Markdown 先变为语义 HTML，再作用一个固定的 `Clean Reading` CSS 主题；通用 HTML 保留完整文档与 `<style>`，微信复制版将同一主题的安全 CSS 计算并内联到 HTML 片段。`doocs/md` 已采用主题 CSS 与 Juice 内联的模式，可借鉴其架构而不引入完整渲染器。

首版只提供一个固定主题，并允许选择一个强调色；不开放自定义 CSS、复杂模板或主题市场。主题覆盖正文、标题、引用、代码、表格、图片与脚注：16px 正文、1.8 行距、适度段落间距、层级清晰但不夸张的标题、浅色引用块和移动端安全的图片/表格。中文字体只使用系统回退栈，不依赖在线字体。

Markdown 内嵌 HTML 默认禁用。未来若支持，必须通过 `rehype-raw` 与 `rehype-sanitize` 处理，不能把脚本或事件属性带入导出物。首版图片只允许可公开访问的远程 URL；若图片不可访问，导出应明确阻止并提示处理，不产生表面成功而实际丢图的发布内容。

## 资料

- [WordPress Posts REST API](https://developer.wordpress.org/rest-api/reference/posts/)
- [remark](https://github.com/remarkjs/remark)
- [remark-gfm](https://github.com/remarkjs/remark-gfm)
- [remark-rehype](https://github.com/remarkjs/remark-rehype)
- [rehype-raw](https://github.com/rehypejs/rehype-raw)
- [rehype-sanitize](https://github.com/rehypejs/rehype-sanitize)
- [doocs/md](https://github.com/doocs/md)
- [doocs/md renderer](https://github.com/doocs/md/blob/main/packages/core/src/renderer/renderer-impl.ts)
- [doocs/md default theme](https://github.com/doocs/md/blob/main/packages/shared/src/configs/theme-css/default.css)
- [doocs/md clipboard export](https://github.com/doocs/md/blob/main/apps/web/src/services/export/clipboard.ts)
- [Juice](https://github.com/Automattic/juice)
- [PostPress](https://github.com/liustack/postpress)
- [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark)
- [markdown-rs](https://github.com/wooorm/markdown-rs)
