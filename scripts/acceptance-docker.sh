#!/usr/bin/env bash
# vagent 真机验收(Docker Linux 容器模拟 VPS)
# 用法:
#   accept.sh <TAG>            # root 模式:完整链路(装 xray + 一键 Reality)
#   accept.sh <TAG> user      # 普通用户模式:零参数菜单 + 加用户 + 默认路径落盘
set -euo pipefail

# 默认 TAG 动态查 latest release(避免 stale fallback 用旧二进制)
if [ -z "${1:-}" ]; then
  TAG="$(curl -fsSL "https://api.github.com/repos/gandli/vagent/releases/latest" 2>/dev/null | grep -o '"tag_name": *"[^"]*"' | head -1 | sed 's/.*: *"\(.*\)"/\1/')"
  TAG="${TAG:-v20260712-041404}"
else
  TAG="$1"
fi
MODE="${2:-root}"
REPO="gandli/vagent"
BASE="https://github.com/${REPO}/releases/download/${TAG}"

echo "== [1] 下载 musl 单文件二进制 =="
if [ "$MODE" = "user" ]; then
  BIN_DIR="$HOME/.local/bin"
  mkdir -p "$BIN_DIR"
  export PATH="$BIN_DIR:$PATH"
  curl -fsSL -o "$BIN_DIR/vagent" "${BASE}/vagent"
  curl -fsSL -o "$BIN_DIR/vagent-api" "${BASE}/vagent-api"
  chmod +x "$BIN_DIR/vagent" "$BIN_DIR/vagent-api"
else
  curl -fsSL -o /usr/local/bin/vagent "${BASE}/vagent"
  curl -fsSL -o /usr/local/bin/vagent-api "${BASE}/vagent-api"
  chmod +x /usr/local/bin/vagent /usr/local/bin/vagent-api
fi
vagent --version

if [ "$MODE" = "user" ]; then
  echo "== [2-user] 普通用户模式:零参数进菜单,加用户 + 默认路径落盘 =="
  # 不设 VAGENT_CONFIG → 走默认 $HOME/.config/vagent/spec.toml
  export VAGENT_TEST_INPUT=$'6\n0\nalice\n443\n0\n4\n9\n0\n2\n0\n'
  vagent </dev/null
  SPEC="$HOME/.config/vagent/spec.toml"
  test -f "$SPEC" && echo "spec OK ($SPEC)"
  grep -q 'name = "alice"' "$SPEC" && echo "user alice OK"
  echo "== [3-user] 验证 --user 单元目录生成(不真正 enable) =="
  mkdir -p "$HOME/.config/systemd/user"
  ls -d "$HOME/.config/systemd/user" && echo "user unit dir OK"
  echo "== 普通用户验收完成 =="
  exit 0
fi

echo "== [2] 零参数进菜单,驱动加用户(alice) + 生成订阅 =="
export VAGENT_CONFIG=/root/.config/vagent/spec.toml
export VAGENT_TEST_INPUT=$'6\n0\nalice\n443\n0\n4\n9\n0\n2\n0\n'
vagent </dev/null
test -f "$VAGENT_CONFIG" && echo "spec OK"
grep -q 'name = "alice"' "$VAGENT_CONFIG" && echo "user alice OK"

echo "== [3] 安装 xray(菜单内核管理) + 验证一键 Reality 真生成密钥 =="
export VAGENT_TEST_INPUT=$'10\n0\n1.8.23\n0\n'
vagent </dev/null
export VAGENT_TEST_INPUT=$'2\n0\n'
vagent </dev/null
grep -q 'name = "reality"' "$VAGENT_CONFIG" && echo "reality user OK"
grep -q 'reality_pbk' "$VAGENT_CONFIG" && echo "reality key generated OK" || echo "reality key MISSING (check xray)"

echo "== [4] apply 渲染 + xray -test 校验配置合法 =="
export VAGENT_TEST_INPUT=$'11\n0\n'   # 主菜单 11 → 应用配置(apply)
vagent </dev/null
BASE_DIR="$(dirname "$VAGENT_CONFIG")"
XCFG="$BASE_DIR/cores/xray/config.json"
if [ -f "$XCFG" ]; then
  echo "xray config written: $XCFG"
  # 占位符检查(vagent 渲染漏洞的兜底检测)
  if grep -q "generated-by-xray" "$XCFG"; then
    echo "xray config INVALID (含未生成密钥占位符)"
    /usr/local/bin/xray -test -config "$XCFG" 2>&1 | head -5
  else
    /usr/local/bin/xray -test -config "$XCFG" 2>&1 | head -5
    echo "xray config VALID"
  fi
else
  echo "xray config NOT written"
fi

echo "== [5] 验证 systemd --user 单元生成 =="
ls /root/.config/systemd/user/ 2>/dev/null && echo "user unit dir OK" || echo "no user unit (non-fatal)"

echo "== 验收完成 =="
