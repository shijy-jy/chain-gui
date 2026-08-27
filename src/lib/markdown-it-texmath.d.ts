declare module 'markdown-it-texmath' {
  import type MarkdownIt from 'markdown-it';
  import type { KatexOptions } from 'katex';

  interface TexmathOptions {
    /** KaTeX 渲染引擎实例 */
    engine: unknown;
    /** 公式定界符风格：'dollars' = $...$ 行内、$$...$$ 独立行 */
    delimiters?: string;
    katexOptions?: KatexOptions;
  }

  /**
   * markdown-it 插件（运行时是单参工厂，markdown-it 会先以 options 调用取插件再挂载）。
   * 类型上按 markdown-it 的 PluginWithOptions 双参形态声明以匹配 md.use(texmath, opts)。
   */
  const texmath: (md: MarkdownIt, options?: TexmathOptions) => void;
  export default texmath;
}
