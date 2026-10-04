// 扫描并剥掉误加的 UTF-8 BOM（PowerShell 5.1 的 Set-Content -Encoding UTF8 会加 BOM）
// 用法：node _scratch/fix_bom.cjs [--apply]
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const APPLY = process.argv.includes('--apply');
const SKIP_DIRS = new Set(['node_modules', 'target', 'dist', '.git', '_scratch', '_shots', '_perf']);
const EXTS = new Set(['.toml', '.ts', '.svelte', '.json', '.cjs', '.mjs', '.js', '.md', '.rs', '.bat', '.ps1', '.css', '.html', '.yml', '.yaml']);

const found = [];
function walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) {
      if (!SKIP_DIRS.has(e.name)) walk(path.join(dir, e.name));
      continue;
    }
    const ext = path.extname(e.name).toLowerCase();
    if (!EXTS.has(ext)) continue;
    const p = path.join(dir, e.name);
    const fd = fs.openSync(p, 'r');
    const buf = Buffer.alloc(3);
    fs.readSync(fd, buf, 0, 3, 0);
    fs.closeSync(fd);
    if (buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf) found.push(p);
  }
}
walk(ROOT);

if (found.length === 0) {
  console.log('未发现 BOM ✓');
  process.exit(0);
}

console.log(`发现 ${found.length} 个带 BOM 的文件：`);
for (const p of found) {
  const rel = path.relative(ROOT, p).replace(/\\/g, '/');
  if (APPLY) {
    const buf = fs.readFileSync(p);
    fs.writeFileSync(p, buf.subarray(3));
    console.log('  已剥除  ' + rel);
  } else {
    console.log('  ' + rel);
  }
}
if (!APPLY) console.log('\n（加 --apply 实际剥除）');
