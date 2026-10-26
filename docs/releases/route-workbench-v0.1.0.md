# Ghostdehole 路线工作台 v0.1.0

这是属于 Ghostdehole fork 的首个独立路线规划发行版，基于 YanamiNeko/NekoSportsWorldTool 现有地图与路网代码开发。学校没有打卡点也可使用，无需登录校园账号。

## 下载与启动

下载附件 **Neko-Route-Workbench-v0.1.0-Windows-x64.zip**，解压后运行 `neko-route-workbench.exe`。适用于 Windows x64。`SHA256SUMS.txt` 可核对压缩包完整性，包内 `SHA256.txt` 提供程序文件校验值。

## 功能

- 在画布上打点、选点拖动、输入经纬度、排序或删除节点。
- 路线反向、闭环、距离预览、撤销与重做。
- 导入本地 OSM XML 路网，按起点、途经点和终点规划相连道路。
- 保存 JSON 草稿，导入、导出 GPX 路线。
- 文件覆盖确认、原子保存、独立窗口未保存退出提醒。

自动沿道路规划需要先导入当地 OSM XML；没有路网时可手动编辑坐标折线。坐标系统一为 **WGS84**，请勿直接混用高德 GCJ-02 或百度 BD-09 坐标。本次发行仅包含独立路线规划程序，不生成或提交运动成绩。

## 验证与来源

- 24 项核心测试、8 项界面测试、1 项标签布局测试通过，Windows 正式构建成功。
- 尚未完成真实设备手工验证；本次不提供 APK 或其他平台发行包。
- 发布源码：`3dce0c4493a74874903124f90eb19e9cfe1675c3`；与构建提交 `6ba4d62079855b25bff025738c72418228a71af5` 的源码树一致。
- [构建记录](https://github.com/Ghostdehole/NekoSportsWorldTool/actions/runs/40298019430) · [功能 PR](https://github.com/Ghostdehole/NekoSportsWorldTool/pull/1)
- 包内包含示例路线、示例路网、使用说明、fork 对比及测试记录。示例是合成几何，非真实校园地图。

保留原作者 [YanamiNeko](https://github.com/YanamiNeko/NekoSportsWorldTool) 署名和 **CC BY-NC 4.0** 非商业许可。内置 Noto 字体遵循 **SIL OFL 1.1**，许可证随包提供。
