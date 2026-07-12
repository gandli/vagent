#!/usr/bin/env bash
# vagent 一键安装脚本(对标 v2ray-agent install.sh 的体验)。
# 用法(普通用户也行):
#   wget -P ~ -N --no-check-certificate "https://raw.githubusercontent.com/gandli/vagent/main/install.sh" && bash ~/install.sh
#
# 安装后直接运行 vagent 进入交互菜单,所有设定在菜单内完成(无命令行参数)。
# 尽量不要求 root:
#   - root 用户:装到 /usr/local/bin + /etc/vagent + 提示注册 systemd
#   - 普通用户:装到 ~/.local/bin + ~/.config/vagent,不碰 systemd(手动前台跑或 systemd --user)
#
# 合规边界:仅用于授权测试环境 / 自建 VPS。
set -euo pipefail

REPO="gandli/vagent"
VERSION="${1:-latest}"

echo "== vagent 安装器 =="

# 按权限选安装根:root 走系统目录,普通用户走 HOME
if [ "$(id -u)" = "0" ]; then
  BIN_DIR="/usr/local/bin"
  SPEC_DIR="/etc/vagent"
  ROOT_INSTALL=1
else
  BIN_DIR="$HOME/.local/bin"
  SPEC_DIR="$HOME/.config/vagent"
  ROOT_INSTALL=0
fi

# 确保 bin 目录存在并加入 PATH(普通用户)
mkdir -p "$BIN_DIR"
if [ "$ROOT_INSTALL" = "0" ]; then
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) export PATH="$BIN_DIR:$PATH" ;;
  esac
fi

# 解析最新 release 版本(若未指定)
if [ "$VERSION" = "latest" ]; then
  VERSION=$(curl -sL "https://api.github.com/repos/${REPO}/releases/latest" | grep -oE '"tag_name": *"v?[0-9.]+"' | head -1 | grep -oE 'v?[0-9.]+') || true
  [ -z "$VERSION" ] && VERSION="latest"
fi
echo "目标版本: $VERSION"

BASE="https://github.com/${REPO}/releases/download/${VERSION}"

echo "== 下载 musl 单文件二进制 =="
if [ "${SKIP_DOWNLOAD:-0}" = "1" ]; then
  echo "(SKIP_DOWNLOAD=1,使用已存在的二进制)"
else
  curl -sL -o "$BIN_DIR/vagent" "${BASE}/vagent" && chmod +x "$BIN_DIR/vagent"
  curl -sL -o "$BIN_DIR/vagent-api" "${BASE}/vagent-api" && chmod +x "$BIN_DIR/vagent-api"
fi
echo "二进制已安装: $("$BIN_DIR/vagent" --version 2>&1 | head -1)"

echo "== 初始化 spec =="
mkdir -p "$SPEC_DIR"
echo "首次运行 vagent 会自动引导生成默认配置,无需手动 init。"

if [ "$ROOT_INSTALL" = "1" ]; then
  echo "== root 模式 =="
  echo "二进制已装到 /usr/local/bin,配置在 /etc/vagent。"
  echo "运行 vagent 进入菜单,在『服务管理』里安装并启用 systemd 单元:"
  echo "  vagent            # 菜单 → 服务管理 → 安装 xray / api 单元"
  echo "  systemctl daemon-reload && systemctl enable --now vagent-xray"
else
  echo "== 普通用户模式 =="
  echo "运行 vagent 进入菜单,或直接前台启动:"
  echo "  vagent            # 进入菜单 → 安装内核(自动注册 systemd --user)→ 应用配置"
fi

echo ""
echo "== 安装完成 =="
echo "直接运行 vagent 进入交互菜单,所有操作在菜单内完成:"
echo "  vagent            # 进入管理菜单(用户/内核/分流/证书/Reality/订阅/服务/状态/卸载)"
echo ""
echo "再次配置:直接运行 vagent"
