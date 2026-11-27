# 标记插件

[English](../../architecture/TAG.md) · [文档索引](../INDEX.md) · [插件宿主](../../../core/src/plugins/mod.rs) · [计划 029](../../roadmap/PLAN.md#pr-029)

计划 029 对手工操作、已启用的用户脚本，以及其他正在运行的插件，给真实事件、文件、进程、软件、注册表或系统对象，以及已暂存的报文打标记。程序是 `winstrata-strata-tag`。它使用既有的原生帧协议：绝对的 `--manifest` 和 `--state` 路径，一次一帧 JSON。它不增加帧操作、权限，也不增加工作区以外的 crate。没有内置病毒库。

## 登记

只有 `winstrata-strata-tag` 是传给宿主的运行时旁边的普通文件时，`PluginHost::open` 才登记 `strata.tag`。登记项是内置、版本 1.0.0，并且在启用前保持禁用。能力是 `tag.apply`、`tag.query` 和 `tag.policy`。权限是 `event.publish`、`event.subscribe`、`storage.private` 和 `history.read`。事件是 `tag.applied`、`tag.rejected` 和 `tag.late`。第三方 `install` 仍然返回 `builtin_closed`。`uninstall` 仍然返回 `builtin_retained`。没有这个程序时，`enable` 仍然返回 `implementation_absent`。

启用会启动标记进程。暂停和继续保留进程。插件暂停时，`tag.apply` 记录 `paused`，并且不写报文。禁用和停止会回复，把字节 `joined` 写入 `released`，并以 0 退出。

## 标记

宿主是标记判定和报文字节的唯一写入者。插件进程只保存宿主已经做出的判定。调用方的 `actor`、`time_unix_ns` 和 `safety` 会被丢掉。宿主把 `safety` 写成 `none`。没有标记不是安全结论，打上标记也不是。

actor 由调用方决定。来自 `strata.tag` 的调用是 `user`，`actor_id` 是 Core 的当前用户。来自 `strata.mcp` 的调用会是 `agent`。这个调用方没有安装。缺口是 [OI-056](../records/OPEN_ITEMS.md#oi-056)。其他通过 `plugin.call` 到达 `tag.apply` 的调用方是 `plugin`，`actor_id` 是该插件 id。

`kind` 是 `event`、`packet`、`file`、`process`、`software`、`registry` 或 `system`。`subject` 为 1 到 256 字节。`attribution` 是 `observed`、`inferred` 或 `unknown`。`evidence` 为空，或为 1 到 256 字节。报文还需要一个正的 `packet` 编号。`reason`、`source` 和 `confidence` 要么一起为空，要么各自为 1 到 128 字节，没有首尾空白，也没有控制字符。其他组合是 `tag_rejected`。三者都空时，非报文记为 `untagged`；策略允许写入时，报文账本返回同一个代码。

插件把记录追加到 `tags.jsonl`，并覆盖 `last.json`。`tag.query` 返回最后一条匹配的对象或报文。只有该记录的代码是 `applied` 时 `tagged` 才为真。回答里的 `safety` 是 `none`。未知对象返回 `code` `none` 和 `tagged` false。

## 报文策略

报文字节留在宿主的 `PacketLedger`。它们不进入 1,024 字节的帧。`stage_packet` 把字节留在内存，不创建文件。默认策略是 `memory_only`，默认上限是 256 KiB。`tag.policy` 接受 `memory_only` 和 `tagged_only`。它在创建任何文件之前，以 `policy_rejected` 拒绝 `all`、`persist` 和 `persist_all`。没有全量落盘 API。缩小上限会释放最旧的已保留字节。编号仍然存在，之后的 persist 返回 `unavailable`。

`persist` 的顺序不变：未知编号，然后仅内存，然后缺少标记，然后被拒绝的标记，然后已释放、空或被截断的报文，然后 `create_new` 加 `sync`。失败的写入会删掉自己的半截文件。如果报文文件已经写入而随后的插件交换失败，宿主删除该文件。禁用标记插件不能打开全量落盘，因为写入只发生在插件正在运行且未暂停时的 `tag.apply` 里。

## 总线

`applied` 发布 `tag.applied`。`unavailable` 发布 `tag.late`。`untagged`、`memory_only`、`tag_rejected`、`unknown_packet` 和 `paused` 发布 `tag.rejected`。`policy` 不发布。设备是 `local-plugin-host`，提供者是 `strata.tag`。序号是该插件的实时序号。归因为 observed 或 inferred，并带上依据 id，否则为 unknown。只有 actor 是 `user` 时才填写用户字段。完整度是 complete。没有总线时跳过发布，私有记录仍然保留。

## 脚本和插件关联

脚本工作台的 `tag` 操作不启动解释器。一个已启用、并且拥有该脚本自己的 `plugin.call` 授权的脚本返回 `code` `tag_request`。宿主随后以 actor `script`、`actor_id` 为该脚本 id 调用 `tag.apply`。没有该授权时，工作台返回 `permission_denied`，并且不写标记行。脚本插件清单不获得 `plugin.call`。直接对 `strata.script` 调用 `tag.apply` 会失败，因为该清单不提供这个能力。

拥有 `plugin.call` 的第三方插件可以调用 `tag.apply`。宿主把 actor 标成 `plugin`。

## 官方页面

`/tags` 上的官方页面组合既有的 shadcn-admin [Button](../../../packages/ui/vendor/shadcn-admin/src/components/ui/button.tsx)、[Input](../../../packages/ui/vendor/shadcn-admin/src/components/ui/input.tsx) 和 [Badge](../../../packages/ui/vendor/shadcn-admin/src/components/ui/badge.tsx)。它不加入 Fluent UI、Astryx，也不增加新的视觉体系。它不发送 `enable` 或 `grant`。桥命令 `tag-apply` 只在 `strata.tag` 已经运行时转发载荷。尚未选择存储位置时，它返回 `enabled` false 和 `safety` `none`，并且不创建插件目录。没有打开 Windows 上的这个页面。该缺口是 [OI-054](../records/OPEN_ITEMS.md#oi-054)。

## 本计划不做什么

本计划不在插件内从网卡采集，不导出 PCAPNG，不绘制拓扑，也不做网络发现。这些仍属于计划 031。[E2E-04](../../roadmap/ACCEPTANCE.md#e2e-04) 保持开放。未标记连接元数据仍是 [OI-025](../records/OPEN_ITEMS.md#oi-025)。[E2E-05](../../roadmap/ACCEPTANCE.md#e2e-05) 仍属于计划 030。本计划不启动计划 028 或计划 030。本机 Agent 标记仍是 [OI-056](../records/OPEN_ITEMS.md#oi-056)。这台 Linux 编写主机没有执行该插件的 Windows `cargo test --locked --workspace`。该缺口是 [OI-055](../records/OPEN_ITEMS.md#oi-055)。计划 028 保留 OI-052 和 OI-053。本计划不改写它们。

## 验证

证据在[工程记录](../records/STATUS.md)。
