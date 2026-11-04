# ShareWeb V1.0.1 Windows Edition 变更日志

## V1.0.1 — Windows 独立部署版

### 新增

- **跨平台文件系统适配层** (`src/filesystem/`)
  - `FileSystemProvider` trait 与 `FsError` 错误类型
  - `LinuxFileSystemProvider` 实现（POSIX 语义，reparse 始终 false）
  - `WindowsFileSystemProvider` 实现（重解析点检测 via `GetFileAttributesW`）
  - 编译期平台分派 `default_provider()`

- **Windows Service 集成** (`src/windows/`)
  - 对接 SCM（服务控制管理器）
  - 支持 `--service`/`--console`/`--migrate` 启动模式
  - 优雅退出（CancellationToken + ServerHandle.stop）
  - 恢复策略：3 次自动重启（5s/10s/30s）

- **首次启动强制设置管理员密码**
  - `GET /api/setup/status` 查询初始化状态
  - `POST /api/setup/admin` 设置管理员密码（校验强度：≥8位、含大小写字母与数字）
  - `SetupGuard` middleware 拦截未初始化时的业务请求（403 SETUP_REQUIRED）
  - 移除硬编码默认密码 `Admin@2026`

- **PowerShell 部署脚本套件** (`deploy/windows/`)
  - `preflight.ps1` 环境基线采集与预检
  - `install.ps1` 独立安装主流程
  - `uninstall.ps1` 卸载（支持 -KeepData）
  - `upgrade.ps1` 升级（自动备份 + 健康检查 + 回滚）
  - `rollback.ps1` 回滚
  - `verify.ps1` 零侵入差异验证
  - `README.md` 使用说明

- **构建脚本** (`scripts/`)
  - `build-windows.ps1` Windows x64 Release 构建
  - `package-windows.ps1` 安装包打包
  - `.cargo/config.toml` 默认构建目标

- **配置扩展**
  - `AppConfig` 新增 `[platform]` 段（target/service_name/install_dir/service_account）
  - `AppConfig` 新增 `[security]` 段（share_roots/force_admin_setup/log_retention_days/tls_*/jwt_cookie_only）
  - `ServerConfig` 新增 `static_dir`
  - `LogConfig` 新增 `retention_days`

- **测试脚本** (`tests/`)
  - `integration_setup.ps1` 首次启动密码设置测试
  - `integration_basic.ps1` 基础功能集成测试
  - `integration_zero_intrusion.ps1` 零侵入部署验证测试

### 改进

- **路径安全增强** (`src/utils/path_sanitizer.rs`)
  - 平台感知路径分隔符（Windows 接受 `\` 与 `/`）
  - 盘符大小写不敏感比较（`D:\Share` vs `d:\share`）
  - 重解析点预检（符号链接/junction/hardlink 拒绝访问）
  - 改用 `symlink_metadata` 避免 `canonicalize` 解析符号链接

- **数据库迁移**
  - 激活 `002_setup_state.sql` 迁移
  - 引入 `schema_migrations` 版本化迁移机制
  - 支持 `--migrate` 子命令

- **前端静态资源**
  - Actix-Web 内嵌 SPA 服务（`actix-files`）
  - 消除 Nginx 依赖（零侵入）

- **Cargo 依赖**
  - 新增 `actix-files`/`tokio-util`/`sha2`/`tracing-appender`
  - Windows 平台条件依赖 `windows-service`/`windows-sys`
  - `[profile.release]` 优化（LTO/codegen-units=1/strip）

- **配置模板**
  - `config.windows.toml` 修正 `lock_duration_minutes` 30→15
  - 补充 `[security]` 段缺失字段

### 安全

- 移除固定默认密码 `admin/Admin@2026`
- 首次启动强制设置管理员密码
- 路径沙箱防止符号链接/junction 越界
- JWT 不再出现在 URL（配置预留 `jwt_cookie_only`）

### 零侵入式部署约束

- 不修改 Windows Server 2019 已有 PHP/MySQL/IIS/Apache/Nginx/SQL Server 等业务服务
- 不修改现有 SMB 共享定义和 NTFS 权限
- 不修改系统环境变量
- 独立 EXE/Service/SQLite/配置/日志/端口

### 已知限制（V1.0.1 范围）

- 不含 AD 域认证（V1.0.3）
- 不含 Windows 身份模拟（V1.0.4）
- 不修改 NTFS ACL（V1.0.4）
- 不含企业管理后台 UI（V1.0.5）
- HTTPS 配置预留但不强制启用