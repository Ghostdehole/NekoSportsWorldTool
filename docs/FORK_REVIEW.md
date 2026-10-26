# 原仓库与三个 Fork 的来源核对

核对日期：2026-10-26（北京时间）。本记录对应本次独立路线规划工作台的来源审查，不是“四个仓库全部功能已合并”的声明。

本次代码基线是原仓库 `main` 的 `e229abdbab71e377b86632e588e607413af4b93c`。新增模块用于本地设计路线、查看距离、保存和导出路线；GPX 表达的是计划路线，不包含伪造的运动表现、成绩或打卡完成情况。工作台不调用校园运动服务，不提交成绩，也不把路线接入原项目的活动生成或提交链路。原项目仍保留其原有模块，这不意味着本次新增模块增强了那些功能。

## 1. 截图中的比较对象

截图第三个作者名为 **Yunlk**，其中第四个字符是小写字母 `l`，不是大写 `I`。以下 SHA 均通过 GitHub 仓库/提交接口读取，不以仓库页面的“Updated”时间代替代码提交时间。

| 仓库 | 已核对的默认分支 | 完整提交 SHA | 提交日期（UTC） |
| --- | --- | --- | --- |
| [YanamiNeko/NekoSportsWorldTool](https://github.com/YanamiNeko/NekoSportsWorldTool) | `main` | [`e229abdbab71e377b86632e588e607413af4b93c`](https://github.com/YanamiNeko/NekoSportsWorldTool/commit/e229abdbab71e377b86632e588e607413af4b93c) | 2026-10-20 |
| [GAN1112233131/NekoSportsWorldTool](https://github.com/GAN1112233131/NekoSportsWorldTool) | `main` | [`555c059efd49389df35d96fca6e8efb9db3f1b77`](https://github.com/GAN1112233131/NekoSportsWorldTool/commit/555c059efd49389df35d96fca6e8efb9db3f1b77) | 2026-10-24 |
| [DemaciaForge/NekoSportsWorldTool](https://github.com/DemaciaForge/NekoSportsWorldTool) | `main` | [`9ac21e4cddb2a3b71ee7dad17bc58e47af430b4e`](https://github.com/DemaciaForge/NekoSportsWorldTool/commit/9ac21e4cddb2a3b71ee7dad17bc58e47af430b4e) | 2026-10-14 |
| [Yunlk/Neko-SportsWorld-Tool](https://github.com/Yunlk/Neko-SportsWorld-Tool) | `main` | [`6b677e1b4f4f6e0bf7a287a287d937ead5048800`](https://github.com/Yunlk/Neko-SportsWorld-Tool/commit/6b677e1b4f4f6e0bf7a287a287d937ead5048800) | 2026-09-26 |

相对于上述原仓库 SHA，GitHub 的历史比较结果如下。ahead/behind 表示提交历史差异，不等于功能数或缺陷数。

| Fork | ahead | behind | 解释 |
| --- | ---: | ---: | --- |
| GAN1112233131 | 6 | 1 | 与原仓库共享 `36eba0793e040eeb3ba737fc7fe3c81895740d0c`；原仓库随后多出一次合并提交，Fork 则继续增加地图和记录相关修改。 |
| DemaciaForge | 71 | 4 | 与原仓库共享 `69856c91ce54fcb409f995e8300220d7bef5d917`；其 main 是较早的独立整合线，不能整仓覆盖较新的原仓库。 |
| Yunlk | 0 | 6 | 完全处于原仓库历史之内，默认分支没有需要再次移植的独有改动。 |

可复核的固定版本比较：

- [原仓库 → GAN1112233131](https://github.com/YanamiNeko/NekoSportsWorldTool/compare/e229abdbab71e377b86632e588e607413af4b93c...555c059efd49389df35d96fca6e8efb9db3f1b77)
- [原仓库 → DemaciaForge](https://github.com/YanamiNeko/NekoSportsWorldTool/compare/e229abdbab71e377b86632e588e607413af4b93c...9ac21e4cddb2a3b71ee7dad17bc58e47af430b4e)
- [原仓库 → Yunlk](https://github.com/YanamiNeko/NekoSportsWorldTool/compare/e229abdbab71e377b86632e588e607413af4b93c...6b677e1b4f4f6e0bf7a287a287d937ead5048800)

## 2. “默认分支最新”与“仓库其他新分支”的区别

本次同时枚举了这些仓库的分支，避免把 main 快照误称为整个仓库最后一次更新。以下较新的或名称相关的分支也已核对：

| 仓库/分支 | 已核对 SHA | 与该仓库 main 的关系及处理 |
| --- | --- | --- |
| GAN：`fix/campus-record-followup-20261024` | [`d1c2f074100e97278e6b88aa624ec298086290ef`](https://github.com/GAN1112233131/NekoSportsWorldTool/commit/d1c2f074100e97278e6b88aa624ec298086290ef) | 多 2 个提交，差异只有 README；没有额外路线编辑实现。 |
| GAN：`fix/confirmed-bugs-69856c91` | [`36eba0793e040eeb3ba737fc7fe3c81895740d0c`](https://github.com/GAN1112233131/NekoSportsWorldTool/commit/36eba0793e040eeb3ba737fc7fe3c81895740d0c) | 已进入原仓库所选基线，不重复移植。 |
| Demacia：`integration/upstream-main-pr-audit-2026-10-23` | [`08821aef9c50950fb21589c687ca9d7d666ffd34`](https://github.com/DemaciaForge/NekoSportsWorldTool/commit/08821aef9c50950fb21589c687ca9d7d666ffd34) | 是另起的原仓库维护线，不是其 main 的简单后继。读取了该分支的整合报告；报告说明主要保留 CI 治理改动。没有将其报告中的历史测试数当作本次测试结果。 |
| Demacia：`fix/watermark-statusbar-20261023` | [`24f9663a59af9cf76277c91deb3fdbddb769a30e`](https://github.com/DemaciaForge/NekoSportsWorldTool/commit/24f9663a59af9cf76277c91deb3fdbddb769a30e) | 独立 UI 修复分支，改为利用现有状态栏显示水印，避免新增底部面板挤占小屏空间；未整分支合并。 |
| Yunlk：`fix/run-start-time-random` | [`6746a6d37a6f87df657ba8ff2c93621d65bbad12`](https://github.com/Yunlk/Neko-SportsWorld-Tool/commit/6746a6d37a6f87df657ba8ff2c93621d65bbad12) | 比其 main 落后 19 个提交，ahead 为 0；没有更新的独有代码。 |

Demacia 其他历史开发分支已列举，但没有逐个完整代码审计。本记录不宣称穷尽每个未合并分支的所有边缘问题。

## 3. 实际阅读的代码及取舍

### 原仓库与 Yunlk：保留现有架构

本次沿用原仓库的 Rust/egui 项目结构及独立 `route-planner` crate，不新建一份与原项目无关的网站。地图画布、OSM 解析和道路图是现有代码基础。Yunlk 的相关路线规划及 Android 依赖隔离改动已经存在于该基线中，保留即可，不能算作新合并了一套功能。

### GAN：借鉴预览和地图显示的边界处理

已阅读固定版本的 [`src/ui/map.rs`](https://github.com/GAN1112233131/NekoSportsWorldTool/blob/555c059efd49389df35d96fca6e8efb9db3f1b77/src/ui/map.rs)、[`src/ui/road_tools.rs`](https://github.com/GAN1112233131/NekoSportsWorldTool/blob/555c059efd49389df35d96fca6e8efb9db3f1b77/src/ui/road_tools.rs) 以及其 [校园地图说明](https://github.com/GAN1112233131/NekoSportsWorldTool/blob/555c059efd49389df35d96fca6e8efb9db3f1b77/docs/campus-map-integration-2026-10-23.md)。

- 纯地图绘制使用现有投影、视野适配、平移和缩放接口，适合扩展本地选点。
- GAN 新增 `draw_boundary`/`boundary_edges`，抵消相邻三角形共用边，只画校园边界，避免显示内部剖分斜线；绘制时限制在地图画布内。本次独立编辑器未引入该校园三角剖分模型，因此没有为了“合并数量”移植这组未使用函数。
- GAN 的过期预览隔离、取消结果不覆盖当前状态和缓存恢复，是值得保留的交互原则。本次工作台只保存自身路线状态，不复用登录态、学校缓存或提交回执。
- 其地图备用搜索和记录诊断属于不同的功能链。本次没有宣称已移植这些网络与账户功能。

### Demacia：借鉴自由选点和独立路线几何

已阅读固定版本的 [`src/amap.rs`](https://github.com/DemaciaForge/NekoSportsWorldTool/blob/9ac21e4cddb2a3b71ee7dad17bc58e47af430b4e/src/amap.rs)、[`src/track/custom.rs`](https://github.com/DemaciaForge/NekoSportsWorldTool/blob/9ac21e4cddb2a3b71ee7dad17bc58e47af430b4e/src/track/custom.rs)、[`src/ui/run.rs`](https://github.com/DemaciaForge/NekoSportsWorldTool/blob/9ac21e4cddb2a3b71ee7dad17bc58e47af430b4e/src/ui/run.rs) 及 [`crates/route-planner/src/load.rs`](https://github.com/DemaciaForge/NekoSportsWorldTool/blob/9ac21e4cddb2a3b71ee7dad17bc58e47af430b4e/crates/route-planner/src/load.rs)。

- 采用其产品思路：让用户决定起点、终点和路线，不把学校是否存在打卡点作为路线编辑前提。
- 本次以手动打点和导入本地 OSM 后寻路实现独立规划，不复制其 `CustomRing → build_custom → run_full_flow` 调用链；不需要高德 Key，不会把规划路线自动变成运动记录。
- OSM `leisure=track` 的跑道识别是可独立借鉴的地图数据能力；如工作台加载器扩展该标签，只改变工作台加载路径。关系成员应等全部 way 读取完后再解析，防止 XML 元素顺序造成丢失，并避免重复引入同一个 way。
- 不采用 `stitch_components` 自动连接 30 米内不同连通块：距离接近不能证明可步行穿过，围墙、建筑或封闭入口都可能隔断。断开的路网应明确报无法规划，不能自动虚构连接边。
- 不照搬高德请求模块的静默降级：原代码返程请求失败会反转去程，不向用户说明；地址搜索只选择第一个结果。若未来增加在线路线服务，应显式展示候选及降级状态，并从错误中去除 API Key。
- 不照搬 `build_custom` 的单程几何处理：该函数会将未闭合点列补首点闭合，适合原来的模拟器并不代表适合一条真实单程计划路线。本次单程和闭环必须保持明确区分。

### 额外发现：上游 PR #63 不等于三个 Fork 的 main

已读取 [上游 PR #63](https://github.com/YanamiNeko/NekoSportsWorldTool/pull/63) 的描述及变更文件清单。其当时 head 是 Autumn984 仓库的 `2544735b8bd555ff0a4c87c5abc01e9a6b66e077`，包含 `src/ui/draw.rs` 和 `src/ui/tiles.rs`。这是另外一个贡献者的未合并 PR；不能把它描述为 Demacia 最新 main 已有的完整地图打点功能。本次没有直接合并该 PR 的活动生成或提交集成。

## 4. 许可与验证说明

四个仓库读取到的 `LICENSE` blob SHA 均为 `c2c98da8512c19161633b30345662ed9f566bfc4`，内容相同：**CC BY-NC 4.0**。继续保留 YanamiNeko 署名、[原项目链接](https://github.com/YanamiNeko/NekoSportsWorldTool) 及原 LICENSE；衍生作品不得用于商业目的。上表和代码链接用于注明相关贡献者与设计来源。

本次比较是源码与版本历史核对。Fork README 或发布说明记载的测试通过数量，属于对应作者当时的环境，不能证明当前分支已通过相同测试。当前工作台的可用功能、构建产物和实际检查结果以本分支 README 及本次验证记录为准。

本记录没有测试真实成绩提交、账户登录、验证码处理或第三方运动服务，也不保证地图数据覆盖、校园道路开放状态或路线的现实可通行性。导出文件应作为实地运动前的路线计划查看。
