# 路线工作台验证记录

日期：2026-10-26。基线：`e229abdbab71e377b86632e588e607413af4b93c`。

## 本地验证

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked -p route-workbench` | 24 项通过（4 项 OSM + 20 项文档/路由测试） |
| `cargo test --locked --lib route_workbench` | 8 项通过 |
| `cargo test --locked --lib tab_bar_stays_inside_offscreen_phone_and_desktop_frames` | 1 项通过 |
| `cargo check --locked --lib` | 通过 |
| `cargo check --locked --no-default-features --bin nekosportsworldtool` | 通过；原有 CLI 模块的 dead-code 警告仍存在 |
| `git diff --check` | 通过 |

覆盖内容包括：非法经纬度、JSON 版本/坐标系、GPX 命名空间、DTD 拒绝、多个轨迹段拒绝、大小/点数边界、闭合 GPX、按序途经点、单行边、断开路网、距离过远拒绝、OSM 缺失节点与步行权限、跑道标签、打点/平移/拖点/撤销、过期规划结果隔离、原子保存失败时保留旧数据、不对称路线适配和窄屏布局。

## Windows 与可用范围

Windows x64 由 `.github/workflows/route-workbench.yml` 自动运行同组测试并构建独立程序。以本分支对应的 GitHub Actions 运行状态及产物为准；本地 Linux 测试不能代替 Windows 原生试用。

界面测试使用 egui 无窗口输入模拟，不等于在真实 Windows 显卡、文件选择器或 Android 设备上完成操作验证。未测试 APK。没有登录或向校园服务提交数据，没有验证运动成绩相关功能。

## 独立审查

审查过独立入口及调用链：只初始化本地路线页面、主题与字体，不加载校园账号、设备身份、更新检查或 IP 查询。新增模块未连接运动记录生成或成绩提交接口。

审查指出的旧文件写入风险、确认框背景可操作、未保存退出、点分布不均导致视野裁剪和标签数量测试回归，已分别修复并补充针对性检查。
