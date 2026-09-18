//! 逻辑身份（ADR 0002）：跨设备唯一，因此不能由「名字 + 本机时间」派生。
//!
//! 参与同步的每条作者资产都以 [`new_id`] 生成的 UUIDv4 为主键。此前各服务各自
//! 用「输入哈希 + 本机纳秒时间」拼 id：换一台设备那串东西毫无意义，而且两台设备
//! 各自新建同名资产时，只有时间这一点点熵在起作用——一旦撞车，同步会把两条不同
//! 资产合并成一条，且无法挽回。

use uuid::Uuid;

/// 生成一个新的逻辑身份。
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successive_ids_never_repeat() {
        let ids: Vec<String> = (0..1000).map(|_| new_id()).collect();
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "logical ids must never repeat");
    }

    #[test]
    fn id_is_a_canonical_lowercase_uuidv4() {
        let id = new_id();
        let parsed = Uuid::parse_str(&id).expect("id parses back as a UUID");
        assert_eq!(
            id,
            parsed.to_string(),
            "id stays in canonical 8-4-4-4-12 form"
        );
        assert_eq!(
            parsed.get_version_num(),
            4,
            "id must be random, not name-derived"
        );
    }
}
