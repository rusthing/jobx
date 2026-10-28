use derive_setters::Setters;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use typed_builder::TypedBuilder;
use wheel_rs::serde::duration_serde;

/// Executor 配置
#[derive(Debug, Clone, Serialize, Deserialize, Setters, TypedBuilder)]
#[serde(rename_all = "kebab-case")]
pub struct JobxExecutorConfig {
    /// Redis Stream 的 key的前缀，后面跟着任务执行器的编码
    #[serde(default = "executor_key_default")]
    #[builder(default = executor_key_default())]
    pub executor_key: String,
    /// 任务执行器的编码
    pub executor_code: String,
    /// 任务执行器订阅消息分组
    #[serde(default = "executor_group_default")]
    #[builder(default = executor_group_default())]
    pub executor_group: Option<String>,
    /// 扫描间隔
    #[serde(with = "duration_serde", default = "scan_interval_default")]
    #[builder(default = scan_interval_default())]
    pub scan_interval: Duration,
    /// 单次接收消息的最大数量
    #[serde(default = "max_messages_default")]
    #[builder(default = max_messages_default())]
    pub max_messages: usize,
    /// 批量收到多条消息时是否只处理最后一条
    ///
    /// `false`（默认）= 逐条处理：一次 `XREADGROUP` 拿到的每条消息都会各自走
    /// `take` + 执行 + 上报。
    ///
    /// `true` = 只处理最后一条：同一批次里**只有最后一条**会交给 handler 执行，
    /// 其余就地丢弃 —— 需要应答（`noack = false`）时逐条 ACK 掉；免应答
    /// （`noack = true`）时它们从未进过 PEL，连遍历都不做，只打一行汇总日志
    /// （见 `dispatch_stream_messages`）。正常轮询与消费者组维护
    /// （`XAUTOCLAIM` 认领回来的消息）两条路径都生效。
    ///
    /// ## 为什么需要它
    ///
    /// worker 长时间掉线（进程崩溃、机器重启、发布中断）期间，调度器每 `scan_interval`
    /// 都会重发同一个 job，Stream 里会堆积大量**内容完全相同的重复消息**
    /// （`next_assign_ms` 没被推进，payload 就是同一份快照）。worker 恢复后一次
    /// `XREADGROUP` 就能捞回一大把，逐条处理的话：第一条正常 `take`；其余每条都拿
    /// 同一份 payload 去 `take`，撞 `jobx_task` 的
    /// `unique (job_id, scheduled_exec_start_ms)` 约束失败，白跑一次 DB 事务 + 一次
    /// API 往返；期间还要逐条 ACK、逐条打日志。
    ///
    /// 而这类"补跑"消息在业务上**绝大多数只需要"当前立刻执行一次"** —— 积压十几次
    /// 历史唤醒没有任何价值。开 `true` 后积压被压缩成「一次执行 + N 次 ACK」。
    ///
    /// ## 为什么不影响正确性
    ///
    /// 被跳过的消息对应的 `jobx_job.next_assign_ms` 本来就没被推进，
    /// `list_publishable` 的 `NOT EXISTS(jobx_task...)` 条件依然成立，
    /// 调度器下一轮扫描会重新发布。丢掉的只是"这一次多余的唤醒"。
    ///
    /// ⚠️ 代价是**放弃了批次内「每条都执行一次」的语义**。只有同一个执行器订阅的任务
    /// 彼此等价（如定时健康检查、状态同步）时才适合开启；若每条消息代表不同的业务任务，
    /// 开启会导致除最后一条外全部被丢弃。
    #[serde(default)]
    #[builder(default)]
    pub last_only: bool,
    /// 是否免应答（NOACK）
    ///
    /// `false`（默认）= 可靠消费：`XREADGROUP` 让消息进入本消费者的 PEL，处理完 `XACK`；
    /// 实例崩溃留下的未 ACK 消息由 `run_group_maintenance` 用 `XAUTOCLAIM` 认领重跑。
    /// `true` = 免应答：`XREADGROUP ... NOACK`，消息**不进入 PEL**，消费后也不再 `XACK`。
    ///
    /// ## 怎么选
    ///
    /// - `false`：消息有归属、可崩溃恢复，代价是维护 PEL（每轮维护多几次 Redis 往返）。
    ///   只要执行器是「至少跑一次」的语义（如健康检查、数据同步），就用它。
    /// - `true`：省掉 PEL 与消费者维护，但**投递后未处理完就崩溃的消息会直接丢失**，
    ///   不会重投。仅适合「漏跑一次可接受、重复跑有代价」的场景。
    ///
    /// 注意即使 `true` 也不会永久停摆：消息丢了但 `jobx_job.next_assign_ms` 没被推进
    /// （推进只发生在 worker 成功 `take` 时），`list_publishable` 的 `NOT EXISTS(jobx_task...)`
    /// 依然成立，调度器下一轮扫描会重新发布。所以 `noack` 丢的只是「这一次唤醒」，
    /// 不是「这个任务」。
    ///
    /// `noack` 为 `true` 时 `run_group_maintenance` 的认领步骤必然扫不到东西
    /// （PEL 恒为空），但仍会保留：一是回收僵死消费者仍然需要，二是配置热切换
    /// （`false → true`）时能把此前遗留的 PEL 清干净。
    #[serde(default)]
    #[builder(default)]
    pub noack: bool,
    /// 上报执行结果失败时最大重试次数
    #[serde(default = "report_retry_count_default")]
    #[builder(default = report_retry_count_default())]
    pub report_retry_count: u32,
    /// 上报执行结果重试间隔
    #[serde(with = "duration_serde", default = "report_retry_interval_default")]
    #[builder(default = report_retry_interval_default())]
    pub report_retry_interval: Duration,
    /// 消费者组维护周期
    ///
    /// 每隔这么久执行一次消费者组维护：用 `XAUTOCLAIM` 认领「空闲超过
    /// `consumer_stale_duration` 仍未 ACK」的消息重跑，并用 `XGROUP DELCONSUMER`
    /// 回收已经死掉的消费者条目。默认 30s。
    ///
    /// 与 `scan_interval`（拉新消息）解耦：维护是低频的运维动作，没必要跟每轮扫描同频。
    #[serde(with = "duration_serde", default = "maintenance_interval_default")]
    #[builder(default = maintenance_interval_default())]
    pub maintenance_interval: Duration,
    /// 消费者/消息的「僵死」判定阈值
    ///
    /// 两个用途共用同一个值：
    /// 1. `XAUTOCLAIM` 的 `min-idle-time` —— 消息空闲超过该时长才认领重跑；
    /// 2. `XGROUP DELCONSUMER` 的判定条件 —— 非本实例的消费者空闲超过该时长且 PEL 已清空才回收。
    ///
    /// ⚠️ **必须显著大于「单条消息从投递到 ACK 的正常耗时」**。空闲时间是从最后一次投递
    /// 算起的，与消费者是否存活无关；阈值太小会把另一个**仍在运行**的实例正在处理的消息
    /// 抢过来，导致同一任务被并发执行两次。默认 60s。
    #[serde(with = "duration_serde", default = "consumer_stale_duration_default")]
    #[builder(default = consumer_stale_duration_default())]
    pub consumer_stale_duration: Duration,
    /// 停机时等待「正在执行的任务」收尾的最长时间
    ///
    /// 进程收到 SIGTERM / SIGINT 后，执行器不再拉取新消息，并对在途任务做两件事：
    ///
    /// 1. **还在等预定执行时刻的**（`take` 已落库、`exec_start_ms` 尚未写入）：
    ///    直接上报 `Failed`（「执行器重启，任务未执行」）。这类任务一次都还没跑，
    ///    留着只会变成永远停在「运行中」的孤儿记录。
    /// 2. **已经在跑的**：最多再等这么久。跑完就正常上报结果；超时则上报
    ///    `Failed`（「执行器停机，执行被中断」），避免记录被进程退出带走。
    ///
    /// ## 取值
    ///
    /// 默认 5s。**必须小于进程被强杀的时间窗**（k8s 默认
    /// `terminationGracePeriodSeconds = 30s`），也必须小于 robotech 停机钩子的
    /// 单钩子上限（10s）—— 钩子在这段时间里等「在途任务数归零」，如果本值更大，
    /// 钩子会先超时返回、进程随即退出，等待就没有意义了。
    /// 超过本值仍未跑完的任务按「执行被中断」上报失败，剩下的交给服务端超时巡检。
    #[serde(with = "duration_serde", default = "shutdown_grace_default")]
    #[builder(default = shutdown_grace_default())]
    pub shutdown_grace: Duration,
}

fn executor_key_default() -> String {
    "jobx:executor".to_string()
}

fn executor_group_default() -> Option<String> {
    Some("Default".to_string())
}

fn scan_interval_default() -> Duration {
    Duration::from_secs(1)
}

fn max_messages_default() -> usize {
    5
}

fn report_retry_count_default() -> u32 {
    3
}

fn report_retry_interval_default() -> Duration {
    Duration::from_secs(1)
}

fn maintenance_interval_default() -> Duration {
    Duration::from_secs(30)
}

fn consumer_stale_duration_default() -> Duration {
    Duration::from_secs(60)
}

fn shutdown_grace_default() -> Duration {
    Duration::from_secs(5)
}
