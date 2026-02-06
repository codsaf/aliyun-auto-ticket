# aliyun-auto-ticket

阿里云轻量应用服务器自动测速 & 提交工单工具。定时检测带宽是否被限速，支持飞书通知和一键审批提交工单。

## 功能特性

- **定时测速**：通过 Speedtest.net 测速，检测服务器实际下载带宽
- **自动提交工单**：当检测到带宽低于阈值时，自动向阿里云提交工单请求解除限速
- **飞书通知**：测速结果实时推送到飞书群，限速时发送告警
- **审批模式**：限速时发送飞书交互卡片，点击按钮后才提交工单，避免误提交
- **自动提交模式**：也可跳过审批，检测到限速直接提交工单
- **多样化模板**：内置多组工单标题和描述模板，自动嵌入实测速度数据
- **多种运行模式**：支持定时任务、立即执行、仅测速、直接提交等多种模式

## 工作流程

```
定时触发 / 手动触发
        |
    Speedtest 测速
        |
   速度 < 阈值？
    /        \
  是           否
  |            |
  |        飞书通知"测速正常"
  |
auto_submit 开启？
  /        \
 是          否
 |           |
直接提交    发飞书卡片
工单        等待点击审批
             |
          点击"提交工单"
             |
          提交工单
```

## 前置条件

- **Rust 工具链**：需要安装 Rust 编译器（建议 1.70+）
- **阿里云 AccessKey**：需要一个有工单权限的 AccessKey ID 和 Secret
- **Linux 服务器**：建议部署在需要监控的服务器上（也可以部署在其他机器上）

### 安装 Rust

如果还没有安装 Rust，运行以下命令：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 获取阿里云 AccessKey

1. 登录 [阿里云控制台](https://home.console.aliyun.com/)
2. 点击右上角头像 → **AccessKey 管理**
3. 创建一个 AccessKey，记下 **AccessKey ID** 和 **AccessKey Secret**

> **安全提示**：建议使用 RAM 子账号的 AccessKey，仅授予工单相关权限。

## 编译

```bash
git clone https://github.com/你的用户名/aliyun-auto-ticket.git
cd aliyun-auto-ticket

# 编译 release 版本
cargo build --release
```

编译产物在 `target/release/aliyun-auto-ticket`。

> **注意**：编译需要系统安装 `pkg-config` 和 `libssl-dev`（用于 OpenSSL）：
> ```bash
> # Debian / Ubuntu
> sudo apt install pkg-config libssl-dev
>
> # Arch Linux
> sudo pacman -S pkgconf openssl
>
> # CentOS / RHEL
> sudo yum install pkgconfig openssl-devel
> ```

## 配置

将 `config.example.json` 复制为 `config.json` 并修改：

```bash
cp config.example.json config.json
```

### 配置项说明

```json
{
  "access_key_id": "你的 AccessKey ID",
  "access_key_secret": "你的 AccessKey Secret",
  "product_id": 14278,
  "category_id": 80793,
  "ticket_title": "我的香港轻量应用服务器带宽被严重限速，请帮忙检查解锁",
  "ticket_description": "您好，我购买的香港轻量应用服务器带宽为30Mbps。请帮忙检查...",
  "cron_expression": "0 0 6,18 * * *",
  "speed_threshold": 20.0,
  "feishu_webhook_url": "https://open.feishu.cn/open-apis/bot/v2/hook/你的webhook-id",
  "callback_url": "http://你的公网IP:9876",
  "callback_port": 9876,
  "auto_submit": false
}
```

| 字段 | 必填 | 说明 | 默认值 |
|------|------|------|--------|
| `access_key_id` | **是** | 阿里云 AccessKey ID | - |
| `access_key_secret` | **是** | 阿里云 AccessKey Secret | - |
| `product_id` | 否 | 产品 ID（轻量应用服务器为 14278） | 自动查询 |
| `category_id` | 否 | 工单分类 ID | 自动查询 |
| `ticket_title` | 否 | 工单标题（`--submit` 模式使用，自动模式会随机生成） | 内置默认值 |
| `ticket_description` | 否 | 工单描述（`--submit` 模式使用，自动模式会随机生成） | 内置默认值 |
| `cron_expression` | 否 | 定时任务 cron 表达式（6 位，含秒） | `0 0 9 * * *`（每天9点） |
| `speed_threshold` | 否 | 限速判定阈值（Mbps），低于此值视为限速 | `20.0` |
| `feishu_webhook_url` | 否 | 飞书群机器人 Webhook URL | 不通知 |
| `callback_url` | 否 | 审批回调的公网 URL（如 `http://1.2.3.4:9876`） | 不启用审批按钮 |
| `callback_port` | 否 | 回调服务监听端口 | `9876` |
| `auto_submit` | 否 | `true` 时跳过审批直接提交工单 | `false` |

> **提示**：所有配置项也可以通过环境变量设置，环境变量优先级高于配置文件。
> 对应关系：`ALIYUN_ACCESS_KEY_ID`、`ALIYUN_ACCESS_KEY_SECRET`、`TICKET_PRODUCT_ID`、`TICKET_CATEGORY_ID`、`TICKET_TITLE`、`TICKET_DESCRIPTION`、`CRON_EXPRESSION`、`SPEED_THRESHOLD`、`FEISHU_WEBHOOK_URL`、`CALLBACK_URL`、`CALLBACK_PORT`、`AUTO_SUBMIT`

### 查询 product_id 和 category_id

如果不确定 `product_id` 和 `category_id` 填什么，可以用查询模式自动获取：

```bash
./target/release/aliyun-auto-ticket --list
```

程序会列出所有产品和分类，找到轻量应用服务器对应的 ID。

### cron 表达式说明

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

## 使用方法

```bash
# 查看帮助
./aliyun-auto-ticket --help

# 定时任务模式（默认，无参数）
./aliyun-auto-ticket

# 立即执行一次（测速 + 通知）
./aliyun-auto-ticket --now

# 仅测速，不提交工单
./aliyun-auto-ticket --speedtest

# 直接提交工单（跳过测速）
./aliyun-auto-ticket --submit

# 查询产品和分类信息
./aliyun-auto-ticket --list
```

### 各模式详解

#### 定时任务模式（默认）

不带参数启动，程序会：
1. 启动回调 HTTP 服务（用于接收审批回调）
2. 按 cron 表达式定时触发测速
3. 根据测速结果决定是否提交工单
4. 持续运行，按 `Ctrl+C` 退出

#### 立即执行模式（`--now` / `-n`）

立即执行一次测速 + 通知流程，适合手动触发或测试。执行后会等待审批回调（如果有的话）。

#### 仅测速模式（`--speedtest` / `-s`）

只进行测速，输出下载速度，不做任何其他操作。适合验证网络环境。

#### 直接提交模式（`--submit`）

跳过测速，直接提交一张工单。使用配置文件中的标题和描述（不随机化）。

## 飞书通知配置

### 创建飞书群机器人

1. 打开飞书，进入你想接收通知的群聊
2. 点击群设置（右上角 `...`）→ **群机器人** → **添加机器人**
3. 选择 **自定义机器人**
4. 复制 **Webhook 地址**，填入 `config.json` 的 `feishu_webhook_url`

### 通知效果

- **测速正常**：发送文本消息 `✅ 测速正常: 32.81 Mbps（阈值: 20 Mbps）`
- **检测到限速**（审批模式）：发送交互卡片，包含速度信息和「提交工单」按钮
- **检测到限速**（自动模式）：发送文本消息，包含速度信息和工单号
- **测速失败**：发送告警消息，提醒手动检查

### 审批回调配置

如果使用审批模式（`auto_submit` 为 `false`），需要配置 `callback_url`：

- `callback_url`：飞书卡片中按钮跳转的地址，必须是你服务器的公网可访问地址
- `callback_port`：程序监听的端口（默认 9876）

示例：服务器公网 IP 为 `1.2.3.4`，则配置：

```json
{
  "callback_url": "http://1.2.3.4:9876",
  "callback_port": 9876
}
```

> **注意**：需要在服务器防火墙 / 安全组中放行 `callback_port` 端口。

## 部署为系统服务

建议使用 systemd 管理，实现开机自启和自动重启。

### 1. 复制二进制文件

```bash
sudo cp target/release/aliyun-auto-ticket /usr/local/bin/
```

### 2. 创建工作目录

```bash
sudo mkdir -p /etc/aliyun-auto-ticket
sudo cp config.json /etc/aliyun-auto-ticket/
```

### 3. 创建 systemd 服务

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

### 4. 启动服务

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
# 重新编译
cargo build --release

# 替换二进制
sudo cp target/release/aliyun-auto-ticket /usr/local/bin/

# 重启服务
sudo systemctl restart aliyun-auto-ticket
```

## 常见问题

### Q: product_id 和 category_id 填 0 会怎样？

程序会自动通过阿里云 API 查询，找到「轻量应用服务器」对应的产品 ID 和分类 ID。但这会多两次 API 调用，建议查询一次后写入配置文件。

### Q: 测速不准怎么办？

测速使用 Speedtest.net 的服务器，会自动选择延迟最低的服务器进行 10 秒单线程下载测试。结果可能与多线程测速工具（如 Ookla Speedtest）有差异，可以适当调低 `speed_threshold`。

### Q: 飞书没收到通知？

1. 检查 `feishu_webhook_url` 是否正确
2. 确认机器人没有被移出群聊
3. 查看程序日志是否有报错

### Q: 审批按钮点了没反应？

1. 检查 `callback_url` 是否是公网可访问的地址
2. 确认防火墙 / 安全组放行了 `callback_port` 端口
3. 确认程序正在运行

### Q: 可以同时监控多台服务器吗？

目前一个实例监控一台服务器。如需监控多台，可以在每台服务器上分别部署，使用不同的配置文件。

## License

MIT
