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
}
