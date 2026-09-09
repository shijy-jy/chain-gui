import './app.css'
import App from './App.svelte'
import CodeViewer from './lib/CodeViewer.svelte'
import { mount } from 'svelte'

// v2.18 代码骨架新窗口：参数经 initialization_script 注入 window.__CODE_VIEW__
// （WebviewUrl::App 不解析查询串）；同时兼容 ?view=code 查询串路径
const cw = (window as any).__CODE_VIEW__ as { ws?: string; node?: string } | undefined
const q = new URLSearchParams(window.location.search)
let app
if (cw || q.get('view') === 'code') {
  app = mount(CodeViewer, {
    target: document.getElementById('app')!,
    props: { ws: cw?.ws ?? q.get('ws') ?? '', nodeId: cw?.node ?? q.get('node') ?? '' },
  })
} else {
  app = mount(App, {
    target: document.getElementById('app')!,
  })
}

export default app
