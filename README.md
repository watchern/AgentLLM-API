<div align="center">

# Agent2API

**把多家 AI 桌面客户端的登录态，变成一个本地 OpenAI 兼容 API 网关**

[简体中文](./README.md) | [English](./docs/README.en.md) | [繁體中文](./docs/README.zh-Hant.md) | [日本語](./docs/README.ja.md) | [한국어](./docs/README.ko.md) | [Português (BR)](./docs/README.pt-BR.md)

[![license](https://img.shields.io/badge/license-MIT%20%2B%20附加声明-green)](./LICENSE)
[![release](https://img.shields.io/github/v/release/aimod-cc/agent2api?include_prereleases)](https://github.com/aimod-cc/agent2api/releases/latest)
[![docker pulls](https://img.shields.io/docker/pulls/aimodcc/agent2api)](https://hub.docker.com/r/aimodcc/agent2api)
[![stars](https://img.shields.io/github/stars/aimod-cc/agent2api?style=flat&color=yellow)](https://github.com/aimod-cc/agent2api/stargazers)
[![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20Docker-blue)](https://github.com/aimod-cc/agent2api/releases/latest)

[✨ 核心特性](#-核心特性) • [💬 社区与支持](#-社区与支持) • [🤝 支持的提供商](#-支持的提供商) • [🚀 快速开始](#-快速开始) • [🐳 Docker 部署](#-docker-部署) • [📸 界面预览](#-界面预览) • [🔧 开发与构建](#-开发与构建) • [📜 使用声明](#-使用声明)

</div>

---

Agent2API 把多家 AI 桌面客户端里已登录的账号接到一起，共用一套 OpenAI 兼容接口。任何支持自定义 `base_url` 的客户端，填上 `http://127.0.0.1:3065/v1` 就能直接调用这些账号的模型额度 —— **不需要上游 API Key，也不需要改客户端**。

## ✨ 核心特性

- 🔌 **OpenAI 兼容** —— 客户端填个 `base_url` 就能用，Chat / Responses / Messages 三种协议都支持
- 🏪 **内置 17 家提供商** —— WorkBuddy、小浣熊、CatPaw、Qoder、Trae 等开箱即用，也支持自定义提供商
- 🔁 **凭证自动续期** —— Token 到期自动换新，临期凭证可批量刷新，模型目录也不会过期
- 🪂 **多账号轮转** —— 按优先级排队，429 自动换账号，限额冷却期内不空等
- 🛡️ **出网代理** —— 每个账号可配独立出口，出站指纹按官方客户端伪装
- 🔑 **网关安全** —— 多把网关 Key 可分权、登录有人机验证，没配 Key 时默认拒绝请求
- 📊 **请求报表** —— 请求量、成功率、Token 消耗一览，还有缓存命中率与 365 天热力图
- ⏰ **定时任务** —— 自动签到、凭证维护、模型目录刷新，开关与间隔在一页里管理
- 💰 **余额与用量** —— 账号行内直接看余额与用量，支持跨账号并发查询
- 🖥️ **三种形态** —— 桌面端（Windows / macOS / Linux，托盘常驻）、服务端（Windows / Linux，无界面，注册成系统服务）、Docker（面板与网关同端口，amd64 / arm64）；都不需要装 Node，界面六语

## ❤️ 赞助商

<a href="https://proxylane.dev/zh-cn/isp-proxies?utm_source=agent2api&utm_medium=partnership&utm_campaign=agent2api_202610&utm_content=github_readme"><img src="assets/sponsor-proxylane.png" alt="ProxyLane（赞助位）" width="100%" /></a>

<table>

<tr>
<td width="240"><a href="https://proxylane.dev/zh-cn/isp-proxies?utm_source=agent2api&utm_medium=partnership&utm_campaign=agent2api_202610&utm_content=github_readme"><picture><source media="(prefers-color-scheme: dark)" srcset="assets/proxylane-logo-for-dark-bg.png" /><img src="assets/proxylane-logo-for-light-bg.png" alt="ProxyLane" width="220" /></picture></a></td>
<td>感谢 <a href="https://proxylane.dev/zh-cn/isp-proxies?utm_source=agent2api&utm_medium=partnership&utm_campaign=agent2api_202610&utm_content=github_readme">ProxyLane</a> 赞助了本项目！ProxyLane 提供 AI 账号专用的原生静态住宅 IP（Static Residential ISP）：IP 来自真实家庭宽带运营商，Scamalytics 低欺诈评分，每个 IP 独享，不限流量，$3.90 起。在「网络代理」页给每个账号配一个出口即可。首次购买使用优惠码 <code>AGENT2API25</code> 享 75 折。</td>
</tr>

</table>

## 💬 社区与支持

- **QQ 群**：Agent2API 交流群，群号 `1127656296` —— 在 QQ 里搜索群号即可加入，使用答疑与问题反馈都在这里。
- **💖 赞赏**：如果 Agent2API 帮到了你，欢迎[请作者喝杯咖啡](docs/SPONSOR.md)；赞赏完全自愿，不解锁任何功能。
- **🤝 赞助 / 商务合作**：想赞助本项目，或想让你的产品出现在这里？发邮件到 [541675139@qq.com](mailto:541675139@qq.com)。

## 🤝 支持的提供商

支持情况一览（✓ 支持 · ✗ 不支持 · — 无此概念）：

| 平台 | LLM 请求 | Token 自动续期 | 模型列表 | 余额查询 | 签到 | 领取类 |
| --- | :--: | :--: | :--: | :--: | :--: | :--: |
| WorkBuddy 国内版 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| WorkBuddy 国际版 | ✓ | ✓ | ✓ | ✓ | ✗ | — |
| 小浣熊 | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| CatPaw | ✓ | ✗ | ✓ | ✓ | ✗ | — |
| AutoClaw（国内版 / 国际版） | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| Qoder | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| Cline（Free / Pass） | ✓ | ✓ | ✓ | ✓ | — | — |
| Accio（国际版 / 国内版） | ✓ | ✓ | ✓ | ✓ | — | — |
| ZCode（国内版 / 国际版） | ✓ | ✗ | ✗ | ✓ | — | ✓ |
| CodeArts | ✓ | ✓ | ✓ | ✓ | — | ✓ |
| Trae | ✓ | ✓ | ✓ | ✓ | — | — |
| Loomy（讯飞） | ✓ | ✗ | ✓ | ✓ | ✓ | — |
| KukuAI（百度文库） | ✓ | ✗ | ✓ | ✓ | ✓ | — |
| MonkeyCode（长亭科技，国内版 / 国际版） | ✓ | ✗ | ✓ | — | — | — |
| Command Code | ✓ | ✗ | ✓ | — | — | — |
| Antigravity（Google，Gemini / Claude / GPT-OSS） | ✓ | ✓ | ✓ | — | — | — |
| OfficeAce（华为云果办 / OfficeClaw） | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| OrcaTerm（腾讯云遨驰终端） | ✓ | ✓ | ✓ | ✓ | — | — |
| 智谱清言 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| 自定义提供商 | ✓ | — | ✓ | — | — | — |

> 注：模型列表的 ✓ 表示支持在线自动拉取，部分平台另有内置静态表兜底；ZCode 仅有内置静态表。Token 续期列 ✗ 表示该平台没有可用的续期接口（账号在有效期内不受影响）。Qoder 签到仅中国版可用。
>
> **智谱清言**是本项目**唯一支持「访客模式」**的一家：不需要登录，点一下就能换到一份可直接对话的凭证（免费额度由上游按设备发放）。代价是访客账号没有积分体系 —— 上表里的「余额查询 / 签到 / 领取类」，以及**任务中心的新手任务与每日任务**，都只对**登录账号**有效（粘贴令牌或导入客户端登录态），访客会被如实跳过。

`/v1/chat/completions`、`/v1/responses`、`/v1/messages`（含 `count_tokens`）与 `/v1/models` 对所有平台一视同仁；模型映射、全局优先级队列、429 降级、出网代理、出站指纹脱敏与请求报表同样全平台通用。

> **本项目仅供学习与交流使用。** 它以非官方客户端形态复用你自己账号的登录态，可能不符合上游服务的使用条款，风险（含账号被风控、封禁）自负；禁止商用或绕过计费。详见[使用声明](#-使用声明)与 [LICENSE](./LICENSE)。
>
> 本项目与各上游厂商及其官方产品均无关（名单见[使用声明](#-使用声明)）；接口形态来自对各家客户端通信的观察，上游随时可能调整。

## 🚀 快速开始

Releases 页提供六种形态的产物，按你的系统选一个：

| 形态 | 产物 | 界面 | 说明 |
| --- | --- | --- | --- |
| **Windows 桌面端** | `Agent2API_<版本>_x64-setup.exe` | 应用窗口 | NSIS 安装包，默认装到 `C:\Program Files\Agent2API`，安装时需要管理员授权 |
| **macOS 桌面端** | `Agent2API_<版本>_universal.dmg` | 应用窗口 | 通用二进制（Apple Silicon 与 Intel 各编一遍再合并），最低 macOS 10.15 |
| **Linux 桌面端** | `Agent2API_<版本>_amd64.deb` 或 `Agent2API_<版本>_amd64.AppImage` | 应用窗口 | deb 装进系统（依赖 webkit2gtk-4.1 / gtk3）；AppImage 免安装单文件，任意发行版可用 |
| **Windows 服务端**（无 GUI） | `agent2api-server-windows-amd64.zip` | **浏览器面板** | 命令行安装、注册成 Windows 服务，见 [服务端部署](#-服务端部署windows--linux无界面) |
| **Linux 服务端**（无 GUI） | `agent2api-server-linux-<arch>.tar.gz` | **浏览器面板** | 命令行安装、systemd 托管，同一个包也是容器内更新载荷 |
| **Docker** | `aimodcc/agent2api:latest` | **浏览器面板** | amd64 / arm64 双架构，见 [Docker 部署](#-docker-部署) |

> 选哪个：**自己有桌面**（要托盘、开机自启、本机登录态导入）→ 桌面端；
> **服务器上没有图形界面**（Windows Server / Linux 服务器）→ 服务端包；
> **已经跑 Docker** → 镜像。
>
> 桌面端需要图形界面，**装不到无 GUI 的服务器上**。服务端包不链接任何
> WebView / GTK 依赖，一个可执行文件加一个 `ui/` 目录就能跑。

**桌面端**（Windows / macOS / Linux）安装后启动即可，**无需安装 Node 或任何其它运行时**；网关跑在应用进程内，托盘常驻。

1. 首次启动即在应用进程内启动本机网关（端口 3065）并打开主窗口；若检测到旧版本的数据目录或数据文件，会弹窗提示迁移，按指引操作即可。
2. 点「账号」页的「添加账号」，选提供商后按弹窗提示完成登录或填写凭证即可（方式以弹窗为准：网页登录 / 手机验证码 / 粘贴凭证 / 导入本机登录态）。
3. 把 OpenAI 客户端的 `base_url` 填成 `http://127.0.0.1:3065/v1`，`api_key` 随便填（例如 `sk-local`，未启用鉴权时服务端不校验）。

关闭窗口默认最小化到托盘，网关继续在后台转发；彻底退出请在托盘图标右键选「退出」。

> **macOS 首次打开**：dmg 里的应用未做 Apple 代码签名与公证（签名需要付费的 Apple Developer 账号，见 `.github/workflows/build.yml` 的说明），Gatekeeper 会拦下首次启动 —— 到「系统设置 → 隐私与安全性」里手动放行一次即可。
>
> **Linux 桌面端**：托盘依赖 `libayatana-appindicator3`（多数桌面环境已自带）；若桌面环境下托盘图标不显示，装 `libayatana-appindicator3-1` 后重开即可。AppImage 首次运行前需要可执行位（`chmod +x`，程序在「打开安装包」时会自动补）。更新方式：deb 走系统安装器（装的时候会问一次密码），AppImage 是换文件即升级。
>
> **桌面端的差异**：AutoClaw 与智谱清言的「导入本机登录态」只在 Windows 可用（读的是 Windows 的 DPAPI 密文，macOS / Linux 上整段收起）；其余家（小浣熊 / CatPaw / Cline）读的是 `HOME` 下的明文文件，三个平台都可用。托盘、开机自启、局域网访问与网页面板托管在 macOS / Linux 上同样支持。

### 验证

网关起来后，用 curl 确认联通性（`model` 填 `GET /v1/models` 里任一实际存在的名字）：

```bash
curl http://127.0.0.1:3065/health
curl http://127.0.0.1:3065/v1/models

curl http://127.0.0.1:3065/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"deepseek-v4.1-flash","messages":[{"role":"user","content":"你好"}],"stream":true}'
```

客户端接入示例（Python SDK）：

```python
from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:3065/v1", api_key="sk-local")
resp = client.chat.completions.create(
    model="deepseek-v4.1-flash",     # 名单里有哪家就是哪家，见 GET /v1/models
    messages=[{"role": "user", "content": "你好"}],
)
print(resp.choices[0].message.content)
```

**浏览器页面**（自建 Web UI 等）用 `fetch` 直连会被跨源预检拒绝——网关默认不应答 CORS，预检落到 API Key 校验上得 401。两种解法：① 设置页「安全 → 网关跨域访问」打开（来源 `*`；此时任何网页都能借本机网关打上游，建议同时配「网关 Key」）；② 页面与网关同源——本地静态服务托管页面并把 `/v1` 反代到 `127.0.0.1:3065`。

### 局域网访问

默认只监听 `127.0.0.1`。开启「设置 → 通用 → 局域网访问」后改听所有网卡，局域网设备把 API 地址指向本机 IP 即可共用（界面会给出完整地址）。出于安全，开启前必须注册面板管理员：管理接口要求管理员会话或网关 Key，无可用 Key 时转发接口同样拒绝服务（开启流程会自动补一把「默认」Key）；也可选择同时开放网页面板（默认关闭）。改动重启生效。

桌面端（Windows / macOS）装包里就带着网页面板那一份 `ui/`，打开「同时开放网页管理面板」后局域网设备用浏览器访问同一地址即是管理界面 —— 与 Docker 版是同一份界面；**桌面窗口自己不走网页登录**（没有登录页），它以本机第一把网关 Key 作为凭证，打开这个开关不需要你额外登录桌面端。网页登录页只在浏览器访问面板时出现。

## 🖥️ 服务端部署（Windows / Linux，无界面）

给**没有图形界面的服务器**用（Windows Server、Linux 服务器、云主机）。产物是同一份
`agent2api-server` 二进制：无窗口、无托盘、不链接 WebView / GTK，运行时不依赖
Node 或任何其它运行时。命令只负责**安装**，装完之后的管理（加账号、建 Key、
看报表）都在浏览器面板里做。

服务以系统服务方式常驻：开机自启、崩溃自动拉起、无控制台后台运行。

### Windows Server

下载 `agent2api-server-windows-amd64.zip`，解压后**以管理员身份**打开 PowerShell：

```powershell
Expand-Archive agent2api-server-windows-amd64.zip -DestinationPath C:\agent2api-pkg
cd C:\agent2api-pkg
.\install-service.ps1
```

脚本会：装程序到 `C:\Agent2API`、数据落到 `C:\ProgramData\Agent2API`、注册名为
`Agent2API` 的 Windows 服务（`LocalSystem`，开机自启）、加防火墙入站规则，然后启动。

```powershell
# 自定义位置与端口
.\install-service.ps1 -InstallDir D:\apps\agent2api -DataDir D:\data\agent2api -Port 8080

# 日常管理
Get-Service Agent2API
Restart-Service Agent2API
Stop-Service Agent2API

# 卸载（保留数据）；加 -Purge 连程序与数据一起删
.\install-service.ps1 -Uninstall
```

**升级**：解压新版本包，重跑一次 `install-service.ps1` 即可（脚本会先停服务、
覆盖程序与界面、再启动；数据与配置不动）。

> **两个 Windows 特有的坑，脚本已经处理好了**：
> 服务进程的工作目录是 `C:\Windows\system32` 而不是 exe 所在目录 —— 界面目录靠
> exe 路径定位，不受影响；数据目录则由脚本显式写入服务注册表的
> `AGENT2API_PROXY_HOME`（不设的话会落到 LocalSystem 的
> `C:\Windows\system32\config\systemprofile` 下，几乎不可能找到）。
> 服务没有控制台，所以日志同时落文件 `C:\ProgramData\Agent2API\logs\server.log`。

### Linux（systemd）

```bash
tar -xzf agent2api-server-linux-amd64.tar.gz
cd agent2api-server-linux-amd64      # 或解包后的目录名
sudo bash install.sh
```

脚本会：建 `agent2api` 系统账号（不允许登录）、装程序到 `/opt/agent2api`、数据落
`/var/lib/agent2api`、配置写到 `/etc/agent2api/agent2api.env`、装 systemd 单元并
`enable + start`。

```bash
# 升级（保留数据与配置，旧二进制备份为 .bak）
sudo bash install.sh --upgrade

# 卸载（保留数据）；加 --purge 连数据与配置一起删
sudo bash install.sh --uninstall

# 日常管理
systemctl status agent2api
systemctl restart agent2api
journalctl -u agent2api -f
```

开机自启、崩溃自动拉起（`Restart=always`）、SIGTERM 优雅停机（等在途请求跑完
再退出，SSE 长连接不会被打断）都已配好。配置改 `/etc/agent2api/agent2api.env`
后 `systemctl restart agent2api` 生效。

> **裸机不要开 `AGENT2API_SELF_UPDATE`**：那是容器内的方案（换文件后退出，靠
> 容器 restart 策略拉起）。裸机上程序文件属 root、服务账号无权替换，单元里已
> 显式设成 `0`。升级走 `install.sh --upgrade`。

### 装完之后

两个平台都一样 —— 打开浏览器：

```
http://<服务器IP>:3065/
```

1. 首次进入会引导**注册管理员账号**（也可以用环境变量预置，见下）
2. 登录后在「网关 Key」页创建一把 API Key
3. 客户端 `base_url` 填 `http://<服务器IP>:3065/v1`，`api_key` 填刚建的那把

> ⚠️ **建 Key 之前 `/v1/*` 会拒绝服务**（fail-closed）。这是刻意的：服务端形态
> 绑 `0.0.0.0`，没配 Key 就把上游额度对全网开放是不可接受的。

### 无人值守部署

管理员和网关 Key 都能用环境变量预置，跳过浏览器里的手动步骤：

| 变量 | 说明 |
| --- | --- |
| `AGENT2API_ADMIN_USER` + `AGENT2API_ADMIN_PASSWORD` | 预置管理员（密码明文填，启动时自动转 bcrypt）；也可用 `AGENT2API_ADMIN_PASSWORD_HASH` 直接给哈希 |
| `AGENT2API_PROXY_API_KEY` | 预置一把网关 Key，跳过「面板里建第一把」 |
| `AGENT2API_PROXY_HOME` | 数据目录 |
| `AGENT2API_HOST` / `AGENT2API_PROXY_PORT` | 监听地址（默认 `0.0.0.0`）/ 端口（默认 `3065`） |
| `AGENT2API_PANEL_PORT` | 面板分端口：管理面单独监听该端口，主端口只留 `/v1/*` |
| `AGENT2API_ALLOW_NO_KEY` | 置 `1` 关闭 fail-closed（仅纯内网） |
| `AGENT2API_LOG_FILE` | 日志文件路径，默认 `{数据目录}/logs/server.log` |
| `AGENT2API_VERBOSE` | 置 `1` 打开 debug 日志 |

Linux 写在 `/etc/agent2api/agent2api.env`；Windows 改服务注册表的
`HKLM\SYSTEM\CurrentControlSet\Services\Agent2API\Environment`（`install-service.ps1`
已经写好基础项，重跑脚本会覆盖）。

> **账号无法预置**：账号需要扫码 / 验证码 / 粘贴凭证，这一步由上游协议决定，
> 绕不过去。批量部署可以把已经配好的**数据目录整个拷过去**（SQLite 库 +
> `account.key` 在同一个目录里），账号随之迁移。

## 🐳 Docker 部署

```bash
docker run -d --name agent2api --restart unless-stopped \
  -p 3065:3065 -v ./data:/data \
  aimodcc/agent2api:latest
```

浏览器打开 `http://<主机>:3065`，首次进入会引导**注册管理员账号**；登录后在「网关 Key」页建一把 API Key 给客户端用 —— `http://<主机>:3065/v1` 即 OpenAI 兼容端点（未建 Key 前拒绝转发）。状态（SQLite 库 / 配置 / 日志）都在 `./data` 一个卷里。

compose 用户（amd64 / arm64 都有镜像）：

```yaml
services:
  agent2api:
    image: aimodcc/agent2api:latest
    container_name: agent2api
    restart: unless-stopped
    ports:
      - "3065:3065"
    volumes:
      - ./data:/data
```

环境变量（都可选）：

| 变量 | 说明 |
| --- | --- |
| `AGENT2API_ADMIN_USER` + `AGENT2API_ADMIN_PASSWORD` | 预置管理员账号密码（密码明文填，启动时自动转哈希）；不填走面板注册 |
| `AGENT2API_PANEL_PORT` | 面板分端口：设后面板（界面 + `/api/*`）单独监听该端口，公网只映射主端口即可把管理面留在内网（面板端口绑回环，写 `127.0.0.1:3066:3066`） |
| `AGENT2API_HOST` / `AGENT2API_PROXY_PORT` | 监听地址（默认 `0.0.0.0`）/ 端口（默认 `3065`） |
| `AGENT2API_ALLOW_NO_KEY` | 置 `1` 关闭 fail-closed（未配 Key 也放行 `/v1`，仅限纯内网） |
| `AGENT2API_CAPTCHA_ENABLED` | 登录页人机验证：默认 `1` 开启，`0` 关闭 |
| `AGENT2API_SELF_UPDATE` | 容器内的更新入口：默认 `1` 开启（面板可「下载并更新」）；置 `0` 关闭，只走手动升级 |
| `WORKBUDDY_UPDATE_REPO` + `WORKBUDDY_UPDATE_API_BASE` | 进阶：把「检查更新 / 下载更新」换成自建源。前者换仓库（默认跟上游 `aimod-cc/agent2api`，fork 后可改），后者换 API 根（默认 `https://api.github.com`，需返回与 GitHub 同形的 release JSON）。配了自建源后，下载白名单只额外放行**该源自己的域名**（按源的 scheme：配 `http://` 就允许明文下载，仅建议本地演练/内网镜像用）|

从源码构建：克隆本仓库后 `docker compose up -d --build`（镜像里只有网关与面板，不含 Rust 工具链）。

**网页面板的功能差异**（源于「没有本机桌面客户端」）：网页登录（WorkBuddy / Qoder / Cline）、手机验证码、粘贴凭证完全可用；需要本机回调的网页登录（AutoClaw / CatPaw / Accio / CodeArts / Trae）与「导入本机登录态」不可用，请改用粘贴凭证。桌面端开着「同时开放网页管理面板」时，用浏览器访问该面板也是这一套差异 —— 同一份界面、同一套判定。

### 容器内更新（可选）

面板的「软件更新」页在容器里能直接「下载并更新」：下载官方 Release 里的 `agent2api-server-linux-<arch>.tar.gz`（校验 `.sha256`）→ 替换可执行文件与 `ui/` → 进程优雅退出、由 `restart` 策略拉起同一个容器。由 `AGENT2API_SELF_UPDATE` 控制（Dockerfile 默认 `1`），置 `0` 则只走手动升级。

同一个 `agent2api-server-linux-<arch>.tar.gz` 也是 [Linux 服务端](#linuxsystemd) 的安装包：包内的 `agent2api-server` 与 `ui/` 是这条更新链按名取的条目，另外还带着 `install.sh` / `agent2api.service` / `agent2api.env.example`。容器更新链只认前两个条目（其余读完即丢，见 `selfupdate::extract_bundle`），所以一份产物同时服务两种用途 —— 容器里解包替换，裸机上解包跑 `install.sh`。

代价要清楚：更新后容器标签与**实际运行的版本会漂移**（`docker inspect` 显示的仍是旧 tag），执行 `docker compose pull && up -d` 重建容器会回到镜像里的版本。想固定版本就别开这个入口。

## 📸 界面预览

### 账号

![账号管理页：全局队列、按模型限额冷却、有效期与余额](./assets/screenshots/accounts.png)

添加账号：

![添加账号：选择提供商与版本，然后走网页登录](./assets/screenshots/add-account.png)

### 模型管理

![模型管理页：按提供商查看模型清单、倍率、上下文与映射](./assets/screenshots/models.png)

### 报表

概览：请求数 / 成功率 / Token 总量 / Top 模型与账号、提供商排名，附用量环图与 365 天活跃热力图：

![报表概览：统计卡片、Top 账号 / 提供商、模型与提供商用量环形图](./assets/screenshots/report-overview.png)

趋势：24 小时**缓存命中率**（折线）与 **Token 消耗**（面积）双轴同图，底部按天 Token 柱状图：

![报表趋势：缓存命中率与 Token 消耗双轴趋势、按天 Token 柱状图](./assets/screenshots/report-trends.png)

### 签到中心

![签到中心页：签到总览与每日签到分组](./assets/screenshots/checkin-center.png)

## 🔧 开发与构建

### 环境要求

- Rust >= 1.77 与 Tauri 2 工具链（编译桌面端本体；Windows 上还需 WebView2 运行时）
- Node.js >= 18.17（只用于 `npm run tauri:*` 与前端构建脚本，桌面端运行时不依赖 Node）
- 目标平台：Windows / macOS / Linux 桌面端、Windows / Linux 服务端（无 GUI）、Docker 服务端

### 常用脚本

```bash
npm run tauri:install      # 安装桌面端依赖（等价 npm --prefix desktop-tauri install）
npm run tauri:dev          # 开发模式调起桌面端（自动热重载）
npm run tauri:build        # 构建桌面端安装包

npm run build:icon         # 生成图标源图（改图标设计后执行，再跑 tauri icon）
```

打包产物为 `target/release/bundle/nsis/Agent2API_<版本>_x64-setup.exe`（3.0 版约 8.6 MB；`src-tauri/.cargo/config.toml` 把 cargo 的 `target-dir` 指到了项目根的 `target/`）。

macOS 的 `.dmg` **只能在 macOS 上构建**（`npm --prefix desktop-tauri run build -- --target universal-apple-darwin`），无法从 Windows 交叉编译，所以正式产物由 CI 的 `build` 工作流产（`macos-latest` + 双目标合并）；Docker 镜像与容器内更新用的服务端包由同一工作流的 `linux-server` job 出（复用 Dockerfile 的编译路径，包内可执行文件与镜像同源同工具链）。

## 📜 使用声明

### 仅供学习与交流

本项目是一个用于学习 HTTP 反向代理、SSE 流式透传、多上游协议适配与桌面端打包（Tauri）等技术主题的实践项目，**仅供个人学习与研究使用**。它不是官方产品，与腾讯公司及 WorkBuddy / CodeBuddy、美团及 CatPaw、商汤及小浣熊、智谱及 AutoClaw / autoglm、阿里巴巴及 Qoder / Accio、华为云及 CodeArts / OfficeAce、字节跳动及 Trae、科大讯飞及 Loomy、百度文库及 KukuAI、长亭科技及 MonkeyCode、Command Code、Google 及 Antigravity 均无任何关联，未获得其授权、认可或赞助。

### 关于反向代理行为

本项目实现的是本地反向代理：在你自己的机器上复用你自己账号的登录态，把请求转发到官方上游网关。它不破解、不绕过任何付费或权限校验，使用的额度始终来自你自己账号本就拥有的配额。但需要明确的是，这种「以非官方客户端形态复用登录态」的转发方式，**可能不符合上游服务的用户协议或使用条款**；是否使用、由此产生的一切后果（包括但不限于账号被限流、被风控、被冻结或封禁），均由使用者自行承担。

### 禁止用途

禁止将本项目用于任何商业用途、二次分发牟利、批量账号运营、绕过上游计费或配额限制、或其他违反当地法律法规的活动。如需在生产环境或商业场景中调用相关模型服务，请使用官方渠道与官方 API。

### 凭证与数据风险

本项目会把账号凭证（`accessToken` / `refreshToken` 等）以**明文**形式保存在本机配置目录（默认 `~/.agent2api/`）中，导出功能生成的文件同样包含明文凭证。请自行妥善保管，切勿提交到公开仓库、上传到网盘或分享给他人。因凭证泄露造成的损失由使用者自行承担。

### 无担保与权利通知

本项目按「现状」提供，作者不对其可用性、稳定性、安全性或对特定用途的适用性作任何承诺。上游接口随时可能变更，本项目可能随时失效而不再维护。完整条款见 [LICENSE](./LICENSE)。本项目中的接口形态、协议字段等信息来自对各官方客户端公开网络通信的观察与整理，相关商标与服务的权利归其各自所有者；若权利方认为本项目存在不当之处，请联系作者，将及时调整或删除。

## 📄 许可

本项目基于 [MIT License](./LICENSE)，可自由使用、修改与分发，须保留版权声明。

需要留意的是：LICENSE 正文之后附有一份**使用声明**，其中第 3 条在 MIT 之上**追加了限制**（禁止商业用途、禁止二次分发牟利、禁止批量账号运营）。因此本项目**不是**纯粹的 MIT 项目——**MIT 条款与使用声明共同构成完整的授权与使用约定**，两者对同一行为给出不同结论时以更严格的一方为准。这也是 `Cargo.toml` 用 `license-file` 指向 LICENSE、而不声明 SPDX `"MIT"` 的原因。

## ⭐ Star History

<a href="https://star-history.com/#aimod-cc/agent2api&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=aimod-cc/agent2api&type=Date&theme=dark" />
    <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=aimod-cc/agent2api&type=Date" />
    <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=aimod-cc/agent2api&type=Date" />
  </picture>
</a>

<div align="center">

**[下载最新版](https://github.com/aimod-cc/agent2api/releases/latest)** • **[问题反馈](https://github.com/aimod-cc/agent2api/issues)**

如果这个项目对你有帮助，欢迎点一个 ⭐️ Star！

</div>
