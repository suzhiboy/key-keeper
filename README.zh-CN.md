# Key Keeper · 账户保险箱

[English](README.md) · **简体中文**

Windows 本地个人资料保险箱。管理登录账号、API 密钥、2FA 和个人卡密；所有记录字段均可留空，允许逐步补全。支持标签、搜索、手动深浅色主题、批量导入和加密备份。

第一版单个保险箱最多保存 1000 条记录。

## 开发与构建

需要 Node.js、Rust stable MSVC、Visual Studio 2022 Build Tools 的「使用 C++ 的桌面开发」工作负载、Windows SDK 和 WebView2。

```powershell
npm ci
npm run tauri dev
```

首次安装 Rust 后需要重新打开终端以刷新 PATH。构建 Windows 安装包：

```powershell
npm run tauri build
```

安装程序输出在 `src-tauri/target/release/bundle/nsis/`。程序运行无需互联网；安装器在系统缺少 WebView2 时需要安装该运行时。项目未配置商业代码签名或自动更新服务。

## 使用

首次打开时设置不少于 12 字符的主密码。主密码无法通过重置找回，请自行妥善保存。

登录账号可以只填写平台、账户名、邮箱、电话号码或密码中的任意部分。API 密钥、卡密和 2FA 也允许先保存草稿；验证码需要有效认证密钥才能生成。2FA 支持 Base32 密钥和 `otpauth://totp` 链接。

密码、密钥和兑换码默认隐藏。复制后 30 秒，如果剪贴板内容仍属于这次复制，程序会清理它；这不等于清除 Windows 剪贴板历史或第三方剪贴板工具的记录。应用内 5 分钟无操作、Windows 锁屏或休眠都会锁定。切换其他应用不会重置本应用的空闲计时。

导入支持 TXT、CSV、XLS/XLSX。先选择类型和文件，指定分隔符、编码、工作表与字段映射，预览后确认导入。TXT 一行一码可映射为「兑换码」。错误行需调整映射/源文件，或明确选择跳过。完全相同的记录默认跳过；同平台同账号的不同内容会作为新记录保留，不覆盖旧资料。Excel 中账号和长卡号建议以文本单元格存储，原文件已丢失的前导零或数字精度无法恢复。

## 数据与备份

保险箱保存在当前 Windows 用户的应用数据目录 `com.keykeeper.desktop/vault.kkv`。整库（包括名称、标签及备注）使用 Argon2id 与 AES-256-GCM 加密。主题偏好不含秘密，单独保存。

备份功能导出自包含的加密 `.kkv` 文件。恢复时使用备份创建时的主密码；修改主密码不会改变已有备份。恢复会整体替换当前保险箱，确认前先备份需要保留的数据。程序不会上传资料，也不会自动备份至云端。

## 检查

```powershell
npm test
npm run typecheck
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

调试版本可用 `KEY_KEEPER_DATA_DIR` 指定合成测试资料目录；发布版本忽略此变量。测试不得使用真实密码或导入资料。

## 仓库约定

`main` 保留起始基线，功能在 `feat/key-keeper-v1` 开发。提交按 `feat`、`fix`、`test`、`docs`、`build` 分类。依赖锁文件提交；真实账号库、备份、导入资料、环境密钥和构建产物不提交。需要临时使用本地资料时放在已忽略的 `private-data/`。

当前范围仅 Windows 单用户、单保险箱。手机端、云同步、浏览器自动填充、附件和团队共享不包含在本版本中。

## 本次验证

2026-11-15：前端 4 项测试、TypeScript 检查、生产构建、Rust 单元和集成测试、Clippy（警告视为错误）、格式检查均通过，已生成 Windows x64 NSIS 安装包。

真实 Windows 调试程序已验证创建保险箱、只填平台和邮箱的记录保存、重启解锁及再次启动还原最小化窗口。浅色与深色布局也已检查。按用户要求暂停后续界面测试：导入、备份恢复、剪贴板定时清理和系统锁屏/休眠的完整桌面交互尚未全部验收；相关数据处理与锁定逻辑已有自动测试覆盖，不能将其等同于完整桌面验收。安装包已构建，未在此机器实际安装。
