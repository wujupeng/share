# 大总理文件管理系统

基于 Web 的 SMB 共享目录文件管理系统，支持文件浏览、上传、下载、预览、OnlyOffice 在线编辑、WebDAV 协议和多用户权限控制。支持 **Windows / Debian 双平台部署**，可作为 **SMB 远程代理** 零侵入式访问生产服务器文件共享。

## 技术栈

| 层级 | 技术 | 版本 |
|------|------|------|
| 后端 | Rust + Actix-Web | rustc 1.99+ / actix-web 4 |
| 前端 | Vue3 + TypeScript + Element Plus | Node.js 20.x / Vite |
| 数据库 | SQLite | 3.46+ (bundled) |
| 文件存储 | 直接操作本地/SMB 挂载目录 | ext4 / cifs / NTFS |
| 在线编辑 | OnlyOffice Document Server | JWT 认证 |
| 协议支持 | WebDAV | PROPFIND / GET / PUT / DELETE |
| 进程管理 | systemd (Linux) / Windows Service | 自动重启 |

## 功能特性

### 文件管理
- **文件浏览**：目录树导航、列表/网格视图切换、OA 风格卡片视图、文件搜索、排序
- **文件上传**：拖拽上传、大文件分片上传（5MB 分片）、上传进度显示
- **文件下载**：单文件下载、批量 ZIP 打包、Range 断点续传
- **文件操作**：新建文件夹、重命名、删除、移动、复制
- **文件预览**：图片、文本、PDF、视频、音频在线预览

### 在线编辑
- **OnlyOffice 集成**：Word/Excel/PowerPoint 在线查看与编辑
- **JWT 安全认证**：三层签名验证（文档配置 → 回调验证 → 保存授权）
- **自动保存**：编辑后自动回写到 SMB 共享目录

### 协议支持
- **WebDAV**：标准 WebDAV 协议，支持 PROPFIND/GET/PUT/DELETE/MKCOL/MOVE
- **第三方挂载**：可通过 WebDAV 客户端直接挂载为本地磁盘

### SMB 远程代理架构
- **零侵入式部署**：不修改生产服务器任何配置，仅通过 SMB/CIFS 协议远程访问
- **Windows 代理**：通过 UNC 路径 + `net use` 访问远程共享
- **Linux 代理**：通过 `mount -t cifs` 挂载远程共享（`sec=ntlmv2` 认证）
- **多共享聚合**：可挂载服务器的所有共享到统一目录，前端显示为顶级目录

### 安全与权限
- **权限控制**：基于角色的访问控制（RBAC）、目录级权限规则、权限继承
- **用户管理**：用户增删改查、角色分配（admin/readwrite/readonly）
- **审计日志**：全操作审计记录（login/download/upload/preview/mkdir/rename/delete/move/copy/onlyoffice_edit/onlyoffice_save），含操作者、来源IP、目标路径、时间戳、结果
- **安全加固**：全接口 JWT 认证、三级角色授权、目录级权限控制、路径遍历防护
- **多终端支持**：JWT Token 认证，多终端可同时登录，各自记录 IP 和操作日志

### 运维特性
- **断电恢复**：systemd Restart=always / Windows Service 自动启动
- **跨平台**：同一代码库编译运行于 Windows Server 和 Debian Linux
- **IP 变化检测**：cron 定时检测，IP 变更自动重启服务

## 部署架构

### 架构一：SMB 远程代理（推荐）

```
用户浏览器 → ShareWeb (代理机) → SMB/CIFS → 生产服务器文件共享
```

**Windows 代理部署（192.168.2.142 → 192.168.2.247）：**
- ShareWeb 以 Windows Service 运行，通过 UNC 路径访问远程共享
- 服务账户需 `SeServiceLogonRight` 权限
- config.toml 中 `base_dir = "\\\\192.168.2.247\\soft\\AutoFileBackup"`

**Debian 代理部署（192.168.2.2 → 192.168.2.247）：**
- ShareWeb 以 systemd 服务运行，通过 `mount -t cifs` 挂载远程共享
- `setcap cap_net_bind_service=+ep` 允许非 root 绑定 80 端口
- mount.cifs 需用 `sec=ntlmv2` 而非 `guest` 选项（内核 CIFS 客户端限制）
- 可批量挂载服务器所有共享，前端显示为顶级目录

### 架构二：本地部署

```
用户浏览器 → Nginx → ShareWeb → 本地/SMB 挂载目录
```

## 项目结构

```
share/
├── src/                        # Rust 后端源码
│   ├── main.rs                 # 入口，路由注册，SMB 连接建立
│   ├── config.rs               # 配置加载（TOML），含 SmbConfig
│   ├── error.rs                # 统一错误定义(ApiError) + ApiResponse
│   ├── handlers/               # HTTP 请求处理器
│   │   ├── auth.rs             # 认证接口（登录/登出/修改密码）
│   │   ├── files.rs            # 文件浏览/操作接口
│   │   ├── upload.rs           # 分片上传接口（init/chunk/complete/status）
│   │   ├── download.rs         # 下载接口（断点续传/ZIP打包）
│   │   ├── preview.rs          # 6类文件预览接口
│   │   ├── users.rs            # 用户管理接口
│   │   ├── permissions.rs      # 权限规则 CRUD 接口
│   │   ├── audit.rs            # 审计日志查询接口
│   │   ├── webdav.rs           # WebDAV 协议接口
│   │   ├── onlyoffice.rs       # OnlyOffice 在线编辑接口
│   │   ├── setup.rs            # 初始化设置接口
│   │   └── health.rs           # 健康检查接口
│   ├── services/               # 业务逻辑层
│   ├── repositories/           # 数据访问层（SQLite）
│   ├── models/                 # 数据模型定义
│   ├── middleware/              # 中间件（认证/RBAC/审计）
│   ├── filesystem/             # 文件系统抽象层
│   │   ├── linux.rs            # Linux 文件系统 provider
│   │   └── windows.rs          # Windows 文件系统 provider
│   ├── windows/                # Windows Service 支持
│   └── utils/
│       ├── path_sanitizer.rs   # 虚拟路径→实际路径映射 + 遍历防护
│       └── file_type.rs        # MIME 类型 + 预览类型判断
├── migrations/                 # 数据库迁移脚本
├── frontend/                   # Vue3 前端
│   ├── src/
│   │   ├── api/                # Axios 实例 + 拦截器
│   │   ├── views/              # 页面组件（OA 风格卡片视图）
│   │   ├── stores/             # Pinia 认证状态
│   │   └── router/             # 路由配置与守卫
│   └── dist/                   # 构建产物
├── config.toml                 # 应用配置文件
└── Cargo.toml                  # Rust 依赖配置
```

## 快速开始

### Debian SMB 代理部署

```bash
# 1. 安装依赖
sudo apt install -y cifs-utils

# 2. 挂载远程共享
sudo mount -t cifs //192.168.2.247/soft /mnt/247 \
  -o username=guest,password=,sec=ntlmv2,vers=2.0,uid=1000,gid=1000

# 3. 编译
cargo build --release

# 4. 配置 config.toml（base_dir 指向挂载点）
# 5. 启动服务
./target/release/web-file-manager
```

### Windows SMB 代理部署

```powershell
# 编译
cargo build --release

# 配置 config.toml
# base_dir = "\\\\192.168.2.247\\soft\\AutoFileBackup"
# 添加 [smb] 段配置远程连接

# 安装为 Windows Service
.\target\release\web-file-manager.exe service
```

### 本地开发

```bash
git clone https://github.com/wujupeng/share.git
cd share

# 后端
cargo run -- config.toml

# 前端
cd frontend && npm install && npm run dev
```

## API 接口

### 认证与用户

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | /api/auth/login | 用户登录 |
| POST | /api/auth/logout | 用户登出 |
| GET | /api/auth/profile | 获取当前用户信息 |
| PUT | /api/auth/password | 修改密码 |
| GET | /api/setup/status | 初始化状态 |
| POST | /api/setup/admin | 设置管理员 |

### 文件操作

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /api/files | 文件列表（分页） |
| GET | /api/files/tree | 目录树 |
| GET | /api/files/search | 文件搜索 |
| POST | /api/files/mkdir | 新建文件夹 |
| PUT | /api/files/rename | 重命名 |
| DELETE | /api/files/delete | 删除 |
| POST | /api/files/move | 移动 |
| POST | /api/files/copy | 复制 |
| GET | /api/files/download | 文件下载（支持 ?token= 参数） |
| POST | /api/files/download/batch | 批量下载（ZIP） |
| GET | /api/files/preview | 文件预览 |

### 上传

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | /api/upload/init | 初始化上传任务 |
| POST | /api/upload/chunk | 上传分片 |
| POST | /api/upload/complete | 完成上传 |
| GET | /api/upload/status | 上传状态查询 |

### OnlyOffice 在线编辑

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /api/files/onlyoffice/config | 获取 OnlyOffice 编辑配置 |
| POST | /api/files/onlyoffice/callback | OnlyOffice 保存回调 |

### WebDAV

| 方法 | 路径 | 说明 |
|------|------|------|
| PROPFIND | /webdav/ | 列出资源属性 |
| OPTIONS | /webdav/ | WebDAV 能力协商 |
| GET | /webdav/ | 下载文件 |
| PUT | /webdav/ | 上传文件 |
| DELETE | /webdav/ | 删除文件 |
| MKCOL | /webdav/ | 创建目录 |
| MOVE | /webdav/ | 移动/重命名 |

### 管理接口（需 admin 角色）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | /api/users | 用户列表 |
| POST | /api/users | 创建用户 |
| PUT | /api/users/{username} | 更新用户 |
| DELETE | /api/users/{username} | 删除用户 |
| GET | /api/permissions | 权限规则列表 |
| POST | /api/permissions | 创建权限规则 |
| DELETE | /api/permissions/{id} | 删除权限规则 |
| GET | /api/audit-logs | 审计日志查询 |
| GET | /api/health | 健康检查 |

## 配置说明

```toml
[server]
host = "0.0.0.0"
port = 80                    # 监听端口
workers = 4                  # 工作线程数
static_dir = "./frontend/dist"  # 前端静态文件目录

[storage]
base_dir = "/mnt/247"        # 共享目录路径（SMB 挂载点或 UNC 路径）
max_file_size = 10737418240  # 最大文件大小（10GB）
chunk_size = 5242880         # 分片大小（5MB）
tmp_dir = "./data/upload_tmp"

[database]
path = "./data/shareweb.db"

[auth]
jwt_secret = "your-secret"   # JWT 签名密钥
jwt_expiration_hours = 24
max_login_attempts = 5       # 最大登录失败次数
lock_duration_minutes = 30   # 账户锁定时长
bcrypt_cost = 12

[log]
level = "info"
file_path = "./logs/app.log"

[security]
force_admin_setup = true     # 首次启动强制设置管理员
share_roots = []             # 允许访问的根路径（空则用 base_dir）

[smb]                        # SMB 远程连接配置（Windows 平台）
remote_path = "\\\\192.168.2.247\\soft"
username = "guest"
password = ""
```

## 角色与权限

| 角色 | 说明 | 权限 |
|------|------|------|
| admin | 管理员 | 所有路径 + 用户管理 + 权限管理 + 日志查看 |
| readwrite | 读写用户 | 配置的路径 + 读写操作 |
| readonly | 只读用户 | 配置的路径 + 只读操作 |

权限规则基于角色配置允许访问的目录路径，支持路径继承。

## 审计日志

每次操作自动记录：

| 字段 | 说明 |
|------|------|
| operator | 操作用户名 |
| action_type | 操作类型（login/download/upload/preview/mkdir/rename/delete/move/copy/onlyoffice_edit/onlyoffice_save） |
| target_path | 操作目标路径 |
| source_ip | 客户端 IP（区分不同终端） |
| result | 操作结果（success/failure） |
| failure_reason | 失败原因 |
| action_time | 操作时间 |

## 运维命令

### Linux (systemd)

```bash
systemctl status shareweb        # 查看服务状态
systemctl restart shareweb       # 重启服务
journalctl -u shareweb -f        # 查看实时日志
```

### Windows (Service)

```powershell
sc.exe query HTKIS-ShareWeb              # 查看服务状态
sc.exe stop HTKIS-ShareWeb               # 停止服务
sc.exe start HTKIS-ShareWeb              # 启动服务
Get-EventLog -LogName Application -Source "HTKIS-ShareWeb" -Newest 20  # 查看日志
```

## 部署记录

### Windows Server 2019 @ 192.168.2.142（SMB 代理）

- 服务名：`HTKIS-ShareWeb`，以 administrator 账户运行
- 通过 UNC 路径访问 `\\192.168.2.247\soft\AutoFileBackup`
- 端口 18888，完整验证通过（文件/WebDAV/OnlyOffice）

### Debian 13 @ 192.168.2.2（SMB 代理）

- systemd 服务 `shareweb.service`，端口 80
- 批量挂载 247 的 43 个共享到 `/mnt/247/` 下
- `mount-247-shares.service` 开机自动挂载所有共享
- 前端显示所有共享作为顶级目录

## 安全架构

```
请求流程：
  浏览器 → [Nginx] → Actix-Web → [JWT认证] → [权限检查] → [审计记录] → 业务处理
                                                    ↓
                                          未认证: 401 缺少认证头
                                          无权限: 403 权限不足
                                          只读用户写操作: 403 需要写入权限
```

- **认证**：所有 API 接口均需 JWT Bearer Token（下载/预览支持 query token 参数）
- **授权**：三级角色（admin/readwrite/readonly），管理接口仅 admin 可访问
- **路径权限**：基于 permission_rules 表的目录级访问控制
- **审计**：全操作记录，含操作者、来源 IP、目标路径、时间戳
- **路径安全**：sanitize_path 防止路径遍历攻击，validate_filename 防止非法文件名
- **OnlyOffice 安全**：JWT 三层签名验证

## 许可证

MIT
