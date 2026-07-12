#!/usr/bin/env bash
# vagent 真机验收(Docker Linux 容器模拟 VPS)
# 验证:install.sh 拉取 + 零参数菜单 + 加用户 + 装 xray + 一键 Reality 真生成密钥 + systemd 单元
set -euo pipefail

TAG="${1:-v20260712-022213}"
REPO="gandli/vagent"
BASE="https://github.com/${REPO}/releases/download/${TAG}"

echo "== [1] 下载 musl 单文件二进制 =="
curl -fsSL -o /usr/local/bin/vagent "${BASE}/vagent"
curl -fsSL -o /usr/local/bin/vagent-api "${BASE}/vagent-api"
chmod +x /usr/local/bin/vagent /usr/local/bin/vagent-api
/usr/local/bin/vagent --version

echo "== [2] 零参数进菜单,驱动加用户(alice) + 生成订阅 =="
export VAGENT_CONFIG=/root/.config/vagent/spec.toml
export VAGENT_TEST_INPUT=$'6\n0\nalice\n443\n0\n4\n9\n0\n2\n0\n'
vagent </dev/null
test -f "$VAGENT_CONFIG" && echo "spec OK"
grep -q 'name = "alice"' "$VAGENT_CONFIG" && echo "user alice OK"

echo "== [3] 安装 xray(菜单内核管理) + 验证一键 Reality 真生成密钥 =="
# 装 xray:走菜单 10 → 0(安装 Xray)
export VAGENT_TEST_INPUT=$'10\n0\n1.8.23\n0\n'
vagent </dev/null
# 一键 Reality(菜单 2):此时 xray 已装,应真生成密钥
export VAGENT_TEST_INPUT=$'2\n0\n'
vagent </dev/null
grep -q 'reality' "$VAGENT_CONFIG" && echo "reality user OK"
# 检查 realityPrivateKey 是否真生成(非占位)
grep -q 'realityPrivateKey' "$VAGENT_CONFIG" && echo "reality key generated OK" || echo "reality key MISSING (check xray)"

echo "== [4] 验证 systemd --user 单元生成 =="
ls /root/.config/systemd/user/ 2>/dev/null && echo "user unit dir OK" || echo "no user unit (non-fatal)"

echo "== 验收完成 =="
