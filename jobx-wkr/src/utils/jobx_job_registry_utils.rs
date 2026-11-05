//! # 计划注册辅助
//!
//! 服务首次启动注册 jobx 计划时，计划需要挂到一个「计划分组」下。分组是给人看的
//! 归类维度（谁负责哪批计划），与执行器、调度周期正交，因此在
//! [`crate::JobxJobConfig`] 里不暴露 `group_id`——部署配置只声明「跑什么、多久跑」，
//! 分组按名称在注册时解析成 ID。
//!
//! 这里只封装两件幂等动作，注册流程仍然由各服务自己按原有方式调用
//! `jobx_api_client.job.add`：
//!
//! - [`ensure_job_group`]：分组不存在就创建，已存在就按名称回查 ID；
//! - [`ensure_existing_job_group`]：计划已存在（重复启动）时，把仍为「未分组」的
//!   计划补到分组下；已经归到其它分组的计划不做覆盖，避免抹掉后台的手工调整。

use anyhow::{anyhow, bail};
use jobx_api::dto::{JobxGroupAddDto, JobxGroupQueryDto, JobxJobModifyDto, JobxJobQueryDto};
use jobx_api_client::get_jobx_api_client;
use robotech::api::RO_CODE_WARNING_DUPLICATE_KEY;
use robotech::api::U64;
use robotech::cst::user_id_cst::SYS_OPERATOR_USER_ID;
use tracing::info;

/// # 解析实际使用的计划分组名称
///
/// 分组名允许由部署配置覆盖，常量只做默认值。配置缺失（空串）或只有空白时
/// 回落到默认常量；否则使用配置值。注册新计划与补齐已存在计划都从这里取名字，
/// 保证「按名称查重」用的始终是同一份值。
pub fn resolve_job_group_name(configured: &str, default_name: &str) -> String {
    let configured = configured.trim();
    if configured.is_empty() {
        default_name.to_string()
    } else {
        configured.to_string()
    }
}

/// # 确保计划分组存在并返回分组 ID
///
/// 先尝试创建；撞唯一键时按名称回查。返回的 ID 用于给新增计划的
/// `group_id` 赋值。
///
/// ## 参数
///
/// - `name`：分组名称（`jobx_group.name`，唯一）；
/// - `remark`：分组备注，仅创建时写入；已存在的分组保持原备注。
///
/// ## 错误
///
/// jobx 客户端未初始化、接口不可用、创建失败、已存在却查不到，都返回错误。
/// 调用方按启动失败处理：计划没挂上分组不致命，但静默降级会让后台分组视图
/// 长期对不上，宁可启动时暴露。
pub async fn ensure_job_group(name: &str, remark: Option<&str>) -> anyhow::Result<u64> {
    let jobx_api_client = get_jobx_api_client()?;
    let add_dto = JobxGroupAddDto {
        name: Some(name.to_string()),
        remark: Some(remark.map(str::to_string)),
        _current_user_id: SYS_OPERATOR_USER_ID.into(),
        ..Default::default()
    };
    match jobx_api_client.group.add(&add_dto).await {
        Ok(ro) if ro.is_ok() => {
            let group = ro
                .extra
                .ok_or_else(|| anyhow!("创建任务分组 {name} 后未返回分组信息"))?;
            info!("任务分组已创建: name={name}, id={}", group.id);
            Ok(group.id.0)
        }
        Ok(ro) if ro.is_warn_code(RO_CODE_WARNING_DUPLICATE_KEY) => {
            let query = JobxGroupQueryDto {
                name: Some(name.to_string()),
                ..Default::default()
            };
            let ro = jobx_api_client
                .group
                .get_by_query_dto(&query, SYS_OPERATOR_USER_ID)
                .await
                .map_err(|e| anyhow!("查询任务分组 {name} 失败: {e:?}"))?;
            let group = ro
                .extra
                .ok_or_else(|| anyhow!("任务分组 {name} 已存在但查询不到"))?;
            info!("任务分组已存在: name={name}, id={}", group.id);
            Ok(group.id.0)
        }
        Ok(ro) => bail!("创建任务分组 {name} 失败: {ro:?}"),
        Err(e) => bail!("创建任务分组 {name} 失败: {e:?}"),
    }
}

/// # 为已存在的计划补齐分组
///
/// 重复启动时 `jobx_job.add` 会撞唯一键，此时计划通常已经在库里。若它还是
/// 「未分组」（`group_id` 为空），把分组补上；若已经归到某个分组，保持现状。
///
/// ## 参数
///
/// - `job_name`：计划名称（`jobx_job.name`，唯一）；
/// - `group_id`：由 [`ensure_job_group`] 解析出的分组 ID。
pub async fn ensure_existing_job_group(job_name: &str, group_id: u64) -> anyhow::Result<()> {
    let jobx_api_client = get_jobx_api_client()?;
    let query = JobxJobQueryDto {
        name: Some(job_name.to_string()),
        ..Default::default()
    };
    let ro = jobx_api_client
        .job
        .get_by_query_dto(&query, SYS_OPERATOR_USER_ID)
        .await
        .map_err(|e| anyhow!("查询计划 {job_name} 失败: {e:?}"))?;
    let job = ro
        .extra
        .ok_or_else(|| anyhow!("计划 {job_name} 已存在但查询不到"))?;
    if job.group_id.is_some() {
        return Ok(());
    }

    let modify_dto = JobxJobModifyDto {
        id: Some(job.id),
        group_id: Some(Some(U64::from(group_id))),
        _current_user_id: SYS_OPERATOR_USER_ID.into(),
        ..Default::default()
    };
    let ro = jobx_api_client
        .job
        .modify(&modify_dto)
        .await
        .map_err(|e| anyhow!("为计划 {job_name} 补齐分组失败: {e:?}"))?;
    if ro.is_ok() {
        info!("已为计划补齐分组: job={job_name}, group_id={group_id}");
        Ok(())
    } else {
        bail!("为计划 {job_name} 补齐分组失败: {ro:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_job_group_name;

    const DEFAULT_GROUP_NAME: &str = "报警中心";

    /// 配置有值时使用配置值
    #[test]
    fn configured_group_name_is_used() {
        assert_eq!(
            resolve_job_group_name("生产报警中心", DEFAULT_GROUP_NAME),
            "生产报警中心"
        );
    }

    /// 配置缺失或为空时回落到默认常量
    #[test]
    fn missing_or_blank_group_name_falls_back_to_default() {
        assert_eq!(resolve_job_group_name("", DEFAULT_GROUP_NAME), DEFAULT_GROUP_NAME);
        assert_eq!(
            resolve_job_group_name("   ", DEFAULT_GROUP_NAME),
            DEFAULT_GROUP_NAME
        );
    }

    /// 配置改动后再次启动仍解析出同一个分组名
    ///
    /// `ensure_job_group` 按名称查重，只要重启时拿到的是同一个名字，第二次启动
    /// 就会命中已存在的分组，不会重复创建。
    #[test]
    fn changed_config_keeps_same_group_name_across_restarts() {
        let configured = "生产报警中心";
        let first_start = resolve_job_group_name(configured, DEFAULT_GROUP_NAME);
        let second_start = resolve_job_group_name(configured, DEFAULT_GROUP_NAME);
        assert_eq!(first_start, second_start);
        assert_eq!(first_start, configured);
        assert_ne!(first_start, DEFAULT_GROUP_NAME);
    }
}
