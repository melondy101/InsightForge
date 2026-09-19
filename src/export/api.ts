import { callCommand } from '../native/command-client';

export interface ClipboardPayload {
  html: string;
  text: string;
}

export const exportApi = {
  /** 同批写入 text/html 与 text/plain，避免纯文本覆盖富文本。 */
  writeClipboard: ({ html, text }: ClipboardPayload) =>
    callCommand<void>('export_write_clipboard', { html, text }),
};
