//! 统一时间戳：跨设备同步要靠时间戳字符串排序判先后，所以全项目只能用一种格式。
//!
//! 定长小数位不是风格问题：变长 `%f` 会让 `...:05.99999Z` 在字典序里排在
//! `...:06.1Z` 之后，`created_at DESC` 于是给出错误顺序（见 references 的排序契约）。

use chrono::{DateTime, Utc};

/// 把时刻渲染成本地存储格式：UTC、定长 6 位小数，字典序等价于时间序。
pub fn format_iso(now: DateTime<Utc>) -> String {
    now.format("%Y-%m-%dT%H:%M:%S.%6fZ").to_string()
}

/// 当前时刻的存储格式。需要确定性时间的测试请注入 [`format_iso`]。
pub fn now_iso() -> String {
    format_iso(Utc::now())
}

/// 把存储格式解析回时刻。小数位宽度不限（3 位与 6 位都能读），
/// 因为历史行里两种精度都真实存在过；解析失败返回 `None`，由调用方决定如何降级。
pub fn parse_iso(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|moment| moment.with_timezone(&Utc))
}

/// `now` 距 `then` 过了多少整分钟（向后时钟给负数）。任一侧读不懂即 `None`。
pub fn minutes_since(then: &str, now: &str) -> Option<i64> {
    let elapsed = parse_iso(now)?.signed_duration_since(parse_iso(then)?);
    Some(elapsed.num_seconds() / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(instant: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(instant)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn renders_utc_with_fixed_width_fraction() {
        assert_eq!(
            format_iso(at("1970-01-01T00:00:00Z")),
            "1970-01-01T00:00:00.000000Z"
        );
        assert_eq!(
            format_iso(at("2024-01-01T00:00:05.9Z")),
            "2024-01-01T00:00:05.900000Z"
        );
    }

    #[test]
    fn cross_second_ordering_survives_lexicographic_compare() {
        assert!(
            format_iso(at("2024-01-01T00:00:05.9Z")) < format_iso(at("2024-01-01T00:00:06.1Z"))
        );
    }

    #[test]
    fn now_iso_has_fixed_width_and_never_goes_backwards() {
        let first = now_iso();
        let second = now_iso();
        assert_eq!(first.len(), 27);
        assert!(first <= second);
    }

    #[test]
    fn reads_both_fraction_widths_that_exist_in_older_rows() {
        // 旧行里混着 SQLite 的 3 位毫秒，新行是 6 位微秒；两者都要能解析。
        assert_eq!(
            parse_iso("2026-01-02T03:04:05.789Z"),
            parse_iso("2026-01-02T03:04:05.789000Z")
        );
        assert_eq!(parse_iso("not a timestamp"), None);
    }

    #[test]
    fn counts_whole_minutes_between_two_stored_timestamps() {
        assert_eq!(
            minutes_since("2026-01-02T03:04:05.000000Z", "2026-01-02T03:19:04.000000Z"),
            Some(14)
        );
        assert_eq!(
            minutes_since("2026-01-02T03:04:05.000000Z", "2026-01-02T03:19:05.000000Z"),
            Some(15)
        );
        // 时钟往回走过：给出负数，由调用方判「还没到点」。
        assert_eq!(
            minutes_since("2026-01-02T03:19:05.000000Z", "2026-01-02T03:04:05.000000Z"),
            Some(-15)
        );
        assert_eq!(
            minutes_since("garbage", "2026-01-02T03:04:05.000000Z"),
            None
        );
    }
}
