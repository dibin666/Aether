use aether_data_contracts::repository::usage::StoredRequestUsageAudit;

pub(super) fn admin_monitoring_usage_is_error(item: &StoredRequestUsageAudit) -> bool {
    item.status_code.is_some_and(|value| value >= 400)
        || item.status.trim().eq_ignore_ascii_case("failed")
        || item.status.trim().eq_ignore_ascii_case("error")
        || item.error_message.is_some()
        || item.error_category.is_some()
}
