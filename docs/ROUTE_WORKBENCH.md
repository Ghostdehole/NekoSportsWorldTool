# Neko 路线工作台

基于 NekoSportsWorldTool 现有 egui 地图和 OSM 路网代码新增的独立路线编辑器。学校没有打卡点也可使用。此模块用于规划、预览和导出路线；不登录校园服务，不提交运动记录，不生成时间、配速、步数或成绩。

## Windows 使用

在本仓库 Actions → **Route Workbench** → 成功运行 → **Artifacts** 中下载 `Neko-Route-Workbench-Windows-x64`，解压，双击 `neko-route-workbench.exe`。

也可以从主程序的「自定义路线」页进入编辑器；独立程序不会初始化主程序的账号、设备身份、自动更新或 IP 查询。

## 自定义路线

1. 输入 WGS84 经度、纬度，添加第一个点。没有地图时画布是坐标示意图；可以先导入本地 OSM XML 路网作为底图。
2. 切换至添加模式，在画布中逐点点击。直线按点击顺序连接；只设置起点和终点不会自动识别中间的真实道路。
3. 编辑模式可选中、移动或删除节点；支持调整顺序、反向、闭合路线和撤销/重做。
4. 查看距离，保存为 JSON 继续编辑，或导出 GPX 路线。GPX 不含运动时间或成绩，兼容性取决于接收软件是否支持 GPX route（`rte/rtept`）。

## 按路网自动规划

导入当地 OSM XML 文件，添加起点、途经点和终点，选择路网规划。规划沿导入路网中的真实连接关系进行；节点会吸附到 100 米内的路网节点。没有附近道路或路网断开时会显示错误，不会以直线伪装成可通行路径。

规划结果依赖 OSM 覆盖和标签质量，仍需结合实际道路、校园出入口与开放情况。路网文件加载在后台进行。不需要地图 API Key，不依赖境外在线底图。

## 坐标与文件

- 本模块统一采用 **WGS84**；不能直接把高德 GCJ-02 或百度 BD-09 坐标当作 WGS84 输入。
- 路线文件最大 5 MiB；本地 OSM 最大 20 MiB；单条路线最多 10000 点。需要额外添加闭合点的 GPX 导出最多允许 9999 个编辑点。
- JSON 保存路线名称、坐标系、是否闭合和有序坐标；不保存账号或令牌。
- GPX 仅导入一条路线或单一轨迹段；多个轨迹段不会被自动跨越连接。
- `examples` 中的路网和路线是赤道附近的合成示例，**不是北京信息科技大学的真实地图**。可以用于离线验证操作。
- 路线和路网由用户手动选择，没有学校位置的默认猜测。
- 独立程序关闭时会提示未保存更改；建议在关闭主程序前手动保存路线草稿。

## 源码构建与检查

```sh
cargo test --locked -p route-workbench
cargo test --locked --lib route_workbench
cargo build --locked --release --bin neko-route-workbench
```

支持图形环境的 Windows/Linux/macOS。提供 Windows x64 自动构建；Android 仅复用主程序中的编辑页面，文件选择能力受平台限制，此次未提供经验证的 APK。

## 来源与许可

保留 [YanamiNeko/NekoSportsWorldTool](https://github.com/YanamiNeko/NekoSportsWorldTool) 的 CC BY-NC 4.0 许可证、署名和非商业限制。各 fork 的审查范围及采用情况见 [FORK_REVIEW.md](FORK_REVIEW.md)，验证结果见 [ROUTE_WORKBENCH_TESTS.md](ROUTE_WORKBENCH_TESTS.md)。路线编辑器是新增模块，不宣称所有 fork 已整体合并。
