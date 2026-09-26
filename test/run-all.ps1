# test/run-all.ps1 — Agent Universe 一键全量回归（Windows）
# 依次跑：JS SDK 单测、历史 bug 回归、Rust gsn-core 全量测试。
$ErrorActionPreference = "Continue"

# 切换到仓库根（本脚本在 test/ 下）
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $root

# 确保能找到 cargo / node（用户级安装，未进全局 PATH 的环境）
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }

$fail = 0

Write-Host "=== 1/3 JS SDK 单元测试 ==="
node js/test/test.js
if ($LASTEXITCODE -ne 0) { $fail++ }

Write-Host ""
Write-Host "=== 2/3 历史 Bug 回归（test/regression.js）==="
node test/regression.js
if ($LASTEXITCODE -ne 0) { $fail++ }

Write-Host ""
Write-Host "=== 3/3 Rust gsn-core 全量测试 ==="
Push-Location gsn-core
try {
  cargo test
  if ($LASTEXITCODE -ne 0) { $fail++ }
} finally { Pop-Location }

Write-Host ""
if ($fail -eq 0) {
  Write-Host "OK 全部测试通过"
  exit 0
} else {
  Write-Host "FAIL 有 $fail 组测试失败"
  exit 1
}
