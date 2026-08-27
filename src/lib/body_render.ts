import MarkdownIt from 'markdown-it';
import texmath from 'markdown-it-texmath';
import katex from 'katex';
import 'katex/dist/katex.min.css';

// v1.9 正文预览渲染：Markdown + LaTeX 数学公式。
// 参照 VS Code Markdown 预览的同款技术栈：markdown-it + markdown-it-texmath + KaTeX。
// - html:false：转义正文里的原始 HTML（本地信任文件也不放行，防脚本注入）
// - linkify/breaks：链接自动识别、单换行即换行（贴近 Obsidian 手感）
const md = new MarkdownIt({ html: false, linkify: true, breaks: true }).use(texmath, {
  engine: katex,
  delimiters: 'dollars',   // $...$ 行内公式、$$...$$ 独立行公式
  katexOptions: { throwOnError: false },   // 公式写错时原样显示而不是抛红错
});

const escapeHtml = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** 把节点正文（Markdown + LaTeX）渲染为 HTML；渲染异常时降级为转义原文 */
export function renderBody(markdown: string): string {
  try {
    return md.render(markdown);
  } catch (e) {
    return `<pre>正文渲染失败：${escapeHtml(String(e))}\n\n${escapeHtml(markdown)}</pre>`;
  }
}
