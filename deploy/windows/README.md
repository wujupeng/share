# ShareWeb Windows Server 2019 部署脚本

## 环境要求

- Windows Server 2019 x64
- PowerShell 5.1+
- .NET Framework 4.8+
- 管理员权限

## 目录结构

```
C:\HTKIS\ShareWeb\
├── bin\shareweb.exe
├── frontend\dist\
├── config\config.toml
├── data\shareweb.db, upload_tmp\, logs\, baseline\
├── certs\server.crt, server.key
└── scripts\
```

## 脚本说明

| 脚本 | 用途 | 退出码 |
|------|------|--------|
| preflight.ps1 | 部署前环境基线采集与预检 | 0/10/11/12/13 |
| install.ps1 | 独立安装主流程 | 0/20/21/22/23 |
| uninstall.ps1 | 卸载（仅 ShareWeb 资源） | 0/30/31 |
| upgrade.ps1 | 升级（自动备份+回滚） | 0/40/41/42 |
| rollback.ps1 | 从备份回滚 | 0/50/51 |
| verify.ps1 | 部署后零侵入差异验证 | 0/60 |

### 退出码语义

| 码 | 含义 |
|----|------|
| 0 | 成功 |
| 10 | 端口冲突 |
| 11 | 服务已存在 |
| 12 | 权限不足 |
| 13 | OS 版本不匹配 |
| 20 | 预检失败 |
| 21 | 服务注册失败 |
| 22 | DB 初始化失败 |
| 23 | 防火墙配置失败 |
| 30 | 服务不存在 |
| 31 | 卸载残留 |
| 40 | 备份失败 |
| 41 | 迁移失败 |
| 42 | 健康检查失败 |
| 50 | 备份不存在 |
| 51 | 回滚迁移失败 |
| 60 | 基线违规 |

## 操作示例

```powershell
# 安装
.\install.ps1 -PackagePath .\shareweb-v1.0.1-windows-x64.zip -Port 18888

# 验证
.\verify.ps1 -InstallDir C:\HTKIS\ShareWeb

# 升级
.\upgrade.ps1 -PackagePath .\shareweb-v1.0.2-windows-x64.zip

# 回滚
.\rollback.ps1 -Timestamp 20260101-120000

# 卸载（保留数据）
.\uninstall.ps1 -KeepData

# 卸载（完全删除）
.\uninstall.ps1
```

## 故障排查

- **端口冲突**：修改 `-Port` 参数或停止占用端口的进程
- **服务名冲突**：修改 `-ServiceName` 参数
- **权限不足**：以管理员身份运行 PowerShell
- **OS 版本不满足**：确认系统为 Windows Server 2019 x64

## 零侵入式部署约束

1. 不修改 Windows Server 2019 已有 PHP/MySQL/IIS/Apache/Nginx/SQL Server 等业务服务
2. 不停止、重启、升级或重新配置任何现有业务服务
3. 不修改现有 SMB 共享定义和 NTFS 权限
4. 不修改系统环境变量及其他业务应用配置
5. ShareWeb 使用独立 EXE、Windows Service、SQLite、配置目录、日志目录及网络端口
6. 卸载和回滚不得删除或修改任何原有业务数据