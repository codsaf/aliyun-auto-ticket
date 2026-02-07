# aliyun-auto-ticket

阿里云轻量应用服务器自动测速 & 提交工单工具。定时检测带宽是否被限速，低于阈值自动提交工单请求解除限速。

支持 **Telegram Bot** 远程操控、**飞书通知**、一键审批等功能。

## 功能特性

- **定时测速**：通过 Cloudflare 测速，检测服务器实际下载带宽
- **自动提交工单**：当检测到带宽低于阈值时，自动向阿里云提交工单请求解除限速
- **Telegram Bot**：在手机上随时发命令测速、查状态、提工单，无需登录服务器
- **飞书通知**：测速结果实时推送到飞书群，限速时发送告警
- **审批模式**：限速时发送交互卡片 / Telegram 按钮，点击确认后才提交工单
- **自动提交模式**：也可跳过审批，检测到限速直接提交工单
- **手动触发**：通过浏览器链接、飞书或 Telegram 随时触发检测
- **多样化模板**：内置多组工单标题和描述模板，自动嵌入实测速度数据
- **多种运行模式**：支持定时任务、立即执行、仅测速、直接提交等多种模式

## 工作流程

```
定时触发 / Telegram /check / 浏览器手动触发
                    |
              Cloudflare 测速
                    |
              速度 < 阈值？
              /           \
            是              否
            |               |
            |          通知"测速正常"
            |        (飞书 + Telegram)
            |
      auto_submit 开启？
        /          \
      是             否
      |              |
   直接提交     发确认按钮/卡片
   工单         (Telegram / 飞书)
                     |
               点击"提交工单"
                     |
                提交工单 → 通知结果
```

## 快速开始

### 1. 准备工作

**必须准备的：**

- **阿里云 AccessKey**：用来调用阿里云 API 提交工单
- **Linux 服务器**：建议部署在你要监控的那台轻量应用服务器上

**可选但推荐：**

- **Telegram Bot Token**：用来在手机上远程操控（强烈推荐）
- **飞书群机器人 Webhook**：用来在飞书群里接收通知

### 2. 获取阿里云 AccessKey

1. 登录 [阿里云控制台](https://home.console.aliyun.com/)
2. 点击右上角头像 → **AccessKey 管理**
3. 创建一个 AccessKey，记下 **AccessKey ID** 和 **AccessKey Secret**

> **安全提示**：建议使用 RAM 子账号的 AccessKey，仅授予工单相关权限。

### 3. 创建 Telegram Bot（推荐）

如果你想在手机上远程操控（测速、提工单、看状态），需要创建一个 Telegram Bot：

**第一步：创建 Bot**

1. 在 Telegram 搜索 `@BotFather`，给它发消息
2. 发送 `/newbot`
3. 按提示输入 Bot 名称和用户名（用户名必须以 `bot` 结尾，比如 `my_aliyun_ticket_bot`）
4. BotFather 会返回一个 **Token**，类似 `123456789:ABCdefGHIjklMNOpqrsTUVwxyz`，记下来

**第二步：获取你的 Chat ID**

1. 在 Telegram 搜索 `@userinfobot`，给它发任意消息
2. 它会回复你的用户信息，其中 `Id` 就是你的 Chat ID（一串数字，比如 `123456789`）

> Chat ID 用来限制只有你能操控 Bot，别人给 Bot 发消息会被忽略。

### 4. 安装

**方式一：下载编译好的（推荐）**

去 [Releases 页面](../../releases) 下载最新版本：

```bash
# 下载（以 v0.1.0 为例）
wget https://github.com/codsaf/aliyun-auto-ticket/releases/download/v0.1.0/aliyun-auto-ticket-linux-amd64.tar.gz

# 解压
tar xzf aliyun-auto-ticket-linux-amd64.tar.gz

# 给执行权限
chmod +x aliyun-auto-ticket
```

**方式二：自己编译**

需要先安装 Rust 编译器：

```bash
# 安装 Rust（如果没装过）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 安装编译依赖（Debian/Ubuntu）
sudo apt install pkg-config libssl-dev

# 克隆并编译
git clone https://github.com/codsaf/aliyun-auto-ticket.git
cd aliyun-auto-ticket
cargo build --release
```

编译产物在 `target/release/aliyun-auto-ticket`。

### 5. 配置

```bash
# 复制示例配置
cp config.example.json config.json

# 编辑配置（用 vim 或 nano 都行）
nano config.json
```

**最简配置**（只填必须的）：

```json
{
  "access_key_id": "你的AccessKey ID",
  "access_key_secret": "你的AccessKey Secret"
}
```

**推荐配置**（带 Telegram Bot）：

```json
{
  "access_key_id": "你的AccessKey ID",
  "access_key_secret": "你的AccessKey Secret",
  "product_id": 14278,
  "category_id": 80793,
  "cron_expression": "0 0 6,18 * * *",
  "speed_threshold": 20.0,
  "auto_submit": false,
  "telegram_bot_token": "你的Bot Token",
  "telegram_chat_id": 你的Chat ID
}
```

**完整配置**（所有功能全开）：

```json
{
  "access_key_id": "你的AccessKey ID",
  "access_key_secret": "你的AccessKey Secret",
  "product_id": 14278,
  "category_id": 80793,
  "ticket_title": "自定义工单标题（可选）",
  "ticket_description": "自定义工单描述（可选）",
  "cron_expression": "0 0 6,18 * * *",
  "speed_threshold": 20.0,
  "feishu_webhook_url": "https://open.feishu.cn/open-apis/bot/v2/hook/你的webhook-id",
  "callback_url": "http://你的公网IP:9876",
  "callback_port": 9876,
  "callback_secret": "改成你自己的随机字符串",
  "auto_submit": false,
  "telegram_bot_token": "你的Bot Token",
  "telegram_chat_id": 你的Chat ID
}
```

### 6. 运行

```bash
# 先测试一下能不能跑
./aliyun-auto-ticket --speedtest

# 没问题的话，启动定时任务模式
./aliyun-auto-ticket
```

## 配置项详解

| 字段 | 必填 | 说明 | 默认值 |
|------|------|------|--------|
| `access_key_id` | **是** | 阿里云 AccessKey ID | - |
| `access_key_secret` | **是** | 阿里云 AccessKey Secret | - |
| `product_id` | 否 | 产品 ID（轻量应用服务器为 14278） | 自动查询 |
| `category_id` | 否 | 工单分类 ID | 自动查询 |
| `ticket_title` | 否 | 工单标题（仅 `--submit` 模式使用，定时任务会随机生成） | 内置默认值 |
| `ticket_description` | 否 | 工单描述（仅 `--submit` 模式使用，定时任务会随机生成） | 内置默认值 |
| `cron_expression` | 否 | 定时任务 cron 表达式（6 位，含秒） | `0 0 9 * * *`（每天 9 点） |
| `speed_threshold` | 否 | 限速判定阈值（Mbps），低于此值视为限速 | `20.0` |
| `feishu_webhook_url` | 否 | 飞书群机器人 Webhook URL | 不通知飞书 |
| `callback_url` | 否 | 审批回调的公网 URL（如 `http://1.2.3.4:9876`） | 不启用飞书审批按钮 |
| `callback_port` | 否 | 回调服务监听端口 | `9876` |
| `callback_secret` | 否 | 回调接口和手动触发的鉴权密钥，防止别人恶意触发 | 不鉴权 |
| `auto_submit` | 否 | `true` 时检测到限速直接提交工单，不需要手动确认 | `false` |
| `telegram_bot_token` | 否 | Telegram Bot Token（通过 @BotFather 获取） | 不启用 Telegram |
| `telegram_chat_id` | 否 | 允许操控 Bot 的 Telegram 用户 ID | 不限制（任何人可用） |

> **提示**：所有配置项也可以通过环境变量设置，环境变量优先级高于配置文件。
> 对应关系：`ALIYUN_ACCESS_KEY_ID`、`ALIYUN_ACCESS_KEY_SECRET`、`TICKET_PRODUCT_ID`、`TICKET_CATEGORY_ID`、`TICKET_TITLE`、`TICKET_DESCRIPTION`、`CRON_EXPRESSION`、`SPEED_THRESHOLD`、`FEISHU_WEBHOOK_URL`、`CALLBACK_URL`、`CALLBACK_PORT`、`CALLBACK_SECRET`、`AUTO_SUBMIT`、`TELEGRAM_BOT_TOKEN`、`TELEGRAM_CHAT_ID`

## Telegram Bot 使用

配置好 `telegram_bot_token` 和 `telegram_chat_id` 后，启动程序（不带参数或用 `--now`），Bot 就会自动上线。

在 Telegram 里搜索你创建的 Bot，发送命令即可操控：

| 命令 | 功能 | 说明 |
|------|------|------|
| `/check` | 立即检测 | 完整流程：测速 → 判断阈值 → 限速则提工单 |
| `/speed` | 仅测速 | 只测速看结果，不触发工单流程 |
| `/submit` | 直接提工单 | 跳过测速直接提交（会有确认按钮） |
| `/status` | 查看状态 | 显示运行时长、上次测速结果、阈值等 |
| `/help` | 帮助 | 显示所有可用命令 |

**使用效果：**

- 发 `/speed`，Bot 回复："正在测速..." → "下载速度: 28.50 Mbps"
- 发 `/check`，如果速度正常，Bot 回复："速度正常: 28.50 Mbps（阈值: 20 Mbps）"
- 发 `/check`，如果被限速且 `auto_submit` 为 `false`，Bot 发送带 **提交工单 / 取消** 两个按钮的消息，你点按钮决定是否提交
- 发 `/status`，Bot 回复运行时长、上次测速结果、定时任务表达式等信息

> 定时任务的测速结果也会同时发送到 Telegram，即使你没主动发命令。

## 命令行用法

```bash
# 查看帮助
./aliyun-auto-ticket --help

# 定时任务模式（默认，不带参数）
./aliyun-auto-ticket

# 立即执行一次（测速 + 通知 + 等待回调）
./aliyun-auto-ticket --now   # 或 -n

# 仅测速，不提交工单
./aliyun-auto-ticket --speedtest   # 或 -s

# 直接提交工单（跳过测速）
./aliyun-auto-ticket --submit

# 查询产品和分类信息
./aliyun-auto-ticket --list   # 或 -l
```

### 各模式说明

| 模式 | 参数 | 说明 |
|------|------|------|
| 定时任务 | 无参数 | 按 cron 表达式定时测速，同时启动回调服务和 Telegram Bot，持续运行 |
| 立即执行 | `--now` / `-n` | 立即执行一次完整流程，然后等待回调，Telegram Bot 同时可用 |
| 仅测速 | `--speedtest` / `-s` | 只测速看结果，程序执行完就退出 |
| 直接提交 | `--submit` | 跳过测速直接提工单，用配置文件中的标题和描述 |
| 查询 | `--list` / `-l` | 查询阿里云产品和分类 ID，方便填写配置 |

## 飞书通知配置

### 创建飞书群机器人

1. 打开飞书，进入你想接收通知的群聊
2. 点击群设置（右上角 `...`）→ **群机器人** → **添加机器人**
3. 选择 **自定义机器人**
4. 复制 **Webhook 地址**，填入 `config.json` 的 `feishu_webhook_url`

### 通知效果

- **测速正常**：发送文本消息 "测速正常: 32.81 Mbps（阈值: 20 Mbps）"
- **检测到限速**（审批模式）：发送交互卡片，包含速度信息和「提交工单」按钮
- **检测到限速**（自动模式）：发送文本消息，包含速度信息和工单号
- **测速失败**：发送告警消息，提醒手动检查

### 审批回调配置

如果使用审批模式（`auto_submit` 为 `false`），需要配置 `callback_url`：

- `callback_url`：飞书卡片中按钮跳转的地址，必须是你服务器的公网可访问地址
- `callback_port`：程序监听的端口（默认 9876）
- `callback_secret`：鉴权密钥，防止其他人通过链接恶意触发你的工单提交

示例：服务器公网 IP 为 `1.2.3.4`，则配置：

```json
{
  "callback_url": "http://1.2.3.4:9876",
  "callback_port": 9876,
  "callback_secret": "你自己设一个随机字符串"
}
```

> **注意**：需要在服务器防火墙 / 安全组中放行 `callback_port` 端口。

### 手动触发检测

定时任务模式下，程序启动后会在日志中打印手动触发链接，形如：

```
手动触发: http://1.2.3.4:9876/check?secret=xxx
```

在浏览器里打开这个链接，就可以立即触发一次完整的测速 + 通知流程。

> 如果配置了 `callback_secret`，链接中会自带鉴权参数，没有密钥的人无法触发。

## 部署为系统服务

建议使用 systemd 管理，实现开机自启和自动重启。

### 1. 复制文件

```bash
# 复制二进制文件
sudo cp target/release/aliyun-auto-ticket /usr/local/bin/

# 创建配置目录
sudo mkdir -p /etc/aliyun-auto-ticket
sudo cp config.json /etc/aliyun-auto-ticket/
```

### 2. 创建 systemd 服务

```bash
sudo tee /etc/systemd/system/aliyun-auto-ticket.service << 'EOF'
[Unit]
Description=Aliyun Auto Ticket
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/etc/aliyun-auto-ticket
ExecStart=/usr/local/bin/aliyun-auto-ticket
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF
```

### 3. 启动服务

```bash
# 重新加载 systemd 配置
sudo systemctl daemon-reload

# 启动服务
sudo systemctl start aliyun-auto-ticket

# 设置开机自启
sudo systemctl enable aliyun-auto-ticket

# 查看运行状态
sudo systemctl status aliyun-auto-ticket

# 查看日志
journalctl -u aliyun-auto-ticket -f
```

### 更新部署

```bash
# 重新编译（或下载新版本）
cargo build --release

# 替换二进制
sudo cp target/release/aliyun-auto-ticket /usr/local/bin/

# 重启服务
sudo systemctl restart aliyun-auto-ticket
```

## cron 表达式说明

本工具使用 **6 位 cron 表达式**（含秒），格式为：

```
秒 分 时 日 月 星期
```

常用示例：

| 表达式 | 含义 |
|--------|------|
| `0 0 9 * * *` | 每天早上 9 点 |
| `0 0 6,18 * * *` | 每天早上 6 点和晚上 6 点 |
| `0 0 */2 * * *` | 每 2 小时 |
| `0 */30 * * * *` | 每 30 分钟 |
| `0 0 9 * * 1-5` | 工作日每天早上 9 点 |

> cron 表达式使用**系统本地时区**。

## 常见问题

### Q: product_id 和 category_id 不知道填什么？

可以用查询模式自动获取：

```bash
./aliyun-auto-ticket --list
```

程序会列出所有产品和分类。也可以填 `0`，程序会自动查询，但每次执行会多两次 API 调用，建议查询一次后写入配置文件。

### Q: 测速不准怎么办？

测速使用 Cloudflare 的下载节点，进行 10 秒单线程下载测试。结果可能与多线程测速工具有差异，可以适当调低 `speed_threshold`。比如买的 30Mbps 带宽，阈值设 20 比较合适。

### Q: Telegram Bot 发了命令没反应？

1. 检查 `telegram_bot_token` 是否正确
2. 检查 `telegram_chat_id` 是否是你的用户 ID（通过 @userinfobot 确认）
3. 确认服务器能访问 Telegram（`curl https://api.telegram.org` 能通就行）
4. 查看程序日志是否有报错

### Q: 飞书没收到通知？

1. 检查 `feishu_webhook_url` 是否正确
2. 确认机器人没有被移出群聊
3. 查看程序日志是否有报错

### Q: 审批按钮点了没反应？

1. 检查 `callback_url` 是否是公网可访问的地址
2. 确认防火墙 / 安全组放行了 `callback_port` 端口
3. 确认程序正在运行

### Q: 担心别人知道链接后恶意触发怎么办？

配置 `callback_secret`，所有回调和手动触发接口都会要求携带密钥参数，不知道密钥的人无法触发。Telegram Bot 则通过 `telegram_chat_id` 限制只有你能操控。

### Q: 可以同时用 Telegram 和飞书吗？

可以。两者互不影响，定时任务的结果会同时发送到两个渠道。Telegram Bot 还可以额外通过命令触发操作。

### Q: 可以同时监控多台服务器吗？

一个实例监控一台服务器。如需监控多台，在每台服务器上分别部署，使用不同的配置文件，可以用同一个 Telegram Bot Token 但配置不同的提醒方式。

## License

MIT
