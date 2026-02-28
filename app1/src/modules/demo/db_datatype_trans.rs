//! SQL转换引擎（规则驱动）
//!
//! 目标：根据来源/目标数据库类型对输入SQL做近似映射，保持接口简洁且易扩展。
//! 扩展：实现 `SqlPairConverter` 并在 `ConversionRegistry::new()` 注册 `(from, to)`。
//! 注意：采用启发式替换，不能覆盖所有差异，生产前请批量验证。
use anyhow::{Result, bail};
use regex::Regex;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// 数据库类型枚举（支持字符串解析）
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DbType {
    Mysql,
    Postgresql,
    Sqlite,
    SqlServer,
    Oracle,
}

impl Display for DbType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DbType::Mysql => write!(f, "mysql"),
            DbType::Postgresql => write!(f, "postgresql"),
            DbType::Sqlite => write!(f, "sqlite"),
            DbType::SqlServer => write!(f, "sqlserver"),
            DbType::Oracle => write!(f, "oracle"),
        }
    }
}

impl FromStr for DbType {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "mysql" => Ok(DbType::Mysql),
            "postgres" | "postgresql" | "pgsql" => Ok(DbType::Postgresql),
            "sqlite" | "sqlite3" => Ok(DbType::Sqlite),
            "sqlserver" | "mssql" => Ok(DbType::SqlServer),
            "oracle" => Ok(DbType::Oracle),
            _ => bail!("unsupported db type: {}", s),
        }
    }
}

/// 一对来源→目标数据库的 SQL 转换器
pub trait SqlPairConverter: Send + Sync {
    fn convert(&self, sql: &str) -> Result<String>;
}

/// 转换器注册中心：集中管理 `(from, to) -> converter`
struct ConversionRegistry {
    map: HashMap<(DbType, DbType), Box<dyn SqlPairConverter>>,
}

impl ConversionRegistry {
    /// 初始化注册表并注册内置转换器
    fn new() -> Self {
        let mut map: HashMap<(DbType, DbType), Box<dyn SqlPairConverter>> = HashMap::new();
        map.insert(
            (DbType::Mysql, DbType::Postgresql),
            Box::new(MysqlToPostgres::default()),
        );
        map.insert(
            (DbType::Postgresql, DbType::Mysql),
            Box::new(PostgresToMysql::default()),
        );
        Self { map }
    }

    /// 执行转换：同库类型直接回传；未注册则报错
    fn convert(&self, sql: &str, from: DbType, to: DbType) -> Result<String> {
        if from == to {
            return Ok(sql.to_string());
        }
        match self.map.get(&(from, to)) {
            Some(conv) => conv.convert(sql),
            None => bail!("converter not found: {} -> {}", from, to),
        }
    }
}

/// MySQL → PostgreSQL 规则集（正则驱动）
struct MysqlToPostgres {
    // 语法/表选项
    re_backtick: Regex,
    re_engine: Regex,
    re_charset: Regex,
    // 整数与自增
    re_auto_inc_int: Regex,
    re_auto_inc_bigint: Regex,
    re_tinyint1: Regex,
    re_tinyint: Regex,
    re_mediumint: Regex,
    re_int: Regex,
    // 浮点/时间
    re_double: Regex,
    re_float: Regex,
    re_datetime: Regex,
    re_unsigned: Regex,
    // 常用函数与分页
    re_now: Regex,
    re_ifnull: Regex,
    re_from_unixtime: Regex,
    re_limit_two: Regex,
    re_on_update_cts: Regex,
}

impl Default for MysqlToPostgres {
    /// 初始化所有规则的正则表达式
    fn default() -> Self {
        Self {
            re_backtick: Regex::new("`").unwrap(),
            re_engine: Regex::new("(?i)ENGINE\\s*=\\s*\\w+").unwrap(),
            re_charset: Regex::new("(?i)DEFAULT\\s*CHARSET\\s*=\\s*\\w+").unwrap(),
            re_auto_inc_int: Regex::new("(?i)\\bINT\\s+AUTO_INCREMENT\\b").unwrap(),
            re_auto_inc_bigint: Regex::new("(?i)\\bBIGINT\\s+AUTO_INCREMENT\\b").unwrap(),
            re_tinyint1: Regex::new("(?i)\\bTINYINT\\s*\\(\\s*1\\s*\\)").unwrap(),
            re_tinyint: Regex::new("(?i)\\bTINYINT\\b").unwrap(),
            re_mediumint: Regex::new("(?i)\\bMEDIUMINT\\b").unwrap(),
            re_int: Regex::new("(?i)\\bINT\\b").unwrap(),
            re_double: Regex::new("(?i)\\bDOUBLE\\b").unwrap(),
            re_float: Regex::new("(?i)\\bFLOAT\\b").unwrap(),
            re_datetime: Regex::new("(?i)\\bDATETIME\\b").unwrap(),
            re_unsigned: Regex::new("(?i)\\bUNSIGNED\\b").unwrap(),
            re_now: Regex::new("(?i)\\bNOW\\s*\\(\\s*\\)").unwrap(),
            re_ifnull: Regex::new("(?i)\\bIFNULL\\s*\\(").unwrap(),
            re_from_unixtime: Regex::new("(?i)\\bFROM_UNIXTIME\\s*\\(").unwrap(),
            re_limit_two: Regex::new("(?i)\\bLIMIT\\s+(\\d+)\\s*,\\s*(\\d+)\\b").unwrap(),
            re_on_update_cts: Regex::new(
                "(?i)DEFAULT\\s+CURRENT_TIMESTAMP\\s+ON\\s+UPDATE\\s+CURRENT_TIMESTAMP",
            )
            .unwrap(),
        }
    }
}

impl SqlPairConverter for MysqlToPostgres {
    fn convert(&self, sql: &str) -> Result<String> {
        let mut out = sql.to_string();
        // 标识符与表选项
        out = self.re_backtick.replace_all(&out, "\"").into_owned();
        out = self.re_engine.replace_all(&out, "").into_owned();
        out = self.re_charset.replace_all(&out, "").into_owned();
        // 整数与自增类型映射
        out = self
            .re_auto_inc_bigint
            .replace_all(&out, "BIGSERIAL")
            .into_owned();
        out = self
            .re_auto_inc_int
            .replace_all(&out, "SERIAL")
            .into_owned();
        out = self.re_tinyint1.replace_all(&out, "BOOLEAN").into_owned();
        out = self.re_tinyint.replace_all(&out, "SMALLINT").into_owned();
        out = self.re_mediumint.replace_all(&out, "INTEGER").into_owned();
        out = self.re_int.replace_all(&out, "INTEGER").into_owned();
        // 浮点与时间
        out = self
            .re_double
            .replace_all(&out, "DOUBLE PRECISION")
            .into_owned();
        out = self.re_float.replace_all(&out, "REAL").into_owned();
        out = self.re_datetime.replace_all(&out, "TIMESTAMP").into_owned();
        out = self.re_unsigned.replace_all(&out, "").into_owned();
        out = self
            .re_on_update_cts
            .replace_all(&out, "DEFAULT CURRENT_TIMESTAMP")
            .into_owned();
        // 常用函数
        out = self
            .re_now
            .replace_all(&out, "CURRENT_TIMESTAMP")
            .into_owned();
        out = self.re_ifnull.replace_all(&out, "COALESCE(").into_owned();
        out = self
            .re_from_unixtime
            .replace_all(&out, "TO_TIMESTAMP(")
            .into_owned();
        // 分页：LIMIT offset, count → LIMIT count OFFSET offset
        out = self
            .re_limit_two
            .replace_all(&out, |caps: &regex::Captures| {
                let off = &caps[1];
                let cnt = &caps[2];
                format!("LIMIT {} OFFSET {}", cnt, off)
            })
            .into_owned();
        Ok(out)
    }
}

/// 便捷入口：将 `sql` 从 `from_db` 转到 `to_db`
///
/// 参数：
/// - `sql`: 原始 SQL
/// - `from_db`: 来源类型（如 `"mysql"`）
/// - `to_db`: 目标类型（如 `"postgresql"`）
pub fn convert_sql(sql: &str, from_db: &str, to_db: &str) -> Result<String> {
    let from = DbType::from_str(from_db)?;
    let to = DbType::from_str(to_db)?;
    let reg = ConversionRegistry::new();
    reg.convert(sql, from, to)
}

struct PostgresToMysql {
    re_dquote: Regex,
    re_bigserial: Regex,
    re_serial: Regex,
    re_boolean: Regex,
    re_integer: Regex,
    re_double_precision: Regex,
    re_real: Regex,
    re_timestamp: Regex,
    re_cts: Regex,
    re_coalesce: Regex,
    re_to_timestamp: Regex,
    re_limit_offset: Regex,
}

impl Default for PostgresToMysql {
    fn default() -> Self {
        Self {
            re_dquote: Regex::new("\"").unwrap(),
            re_bigserial: Regex::new("(?i)\\bBIGSERIAL\\b").unwrap(),
            re_serial: Regex::new("(?i)\\bSERIAL\\b").unwrap(),
            re_boolean: Regex::new("(?i)\\bBOOLEAN\\b").unwrap(),
            re_integer: Regex::new("(?i)\\bINTEGER\\b").unwrap(),
            re_double_precision: Regex::new("(?i)\\bDOUBLE\\s+PRECISION\\b").unwrap(),
            re_real: Regex::new("(?i)\\bREAL\\b").unwrap(),
            re_timestamp: Regex::new("(?i)\\bTIMESTAMP\\b").unwrap(),
            re_cts: Regex::new("(?i)\\bCURRENT_TIMESTAMP\\b").unwrap(),
            re_coalesce: Regex::new("(?i)\\bCOALESCE\\s*\\(").unwrap(),
            re_to_timestamp: Regex::new("(?i)\\bTO_TIMESTAMP\\s*\\(").unwrap(),
            re_limit_offset: Regex::new("(?i)\\bLIMIT\\s+(\\d+)\\s+OFFSET\\s+(\\d+)\\b").unwrap(),
        }
    }
}

impl SqlPairConverter for PostgresToMysql {
    fn convert(&self, sql: &str) -> Result<String> {
        let mut out = sql.to_string();
        out = self.re_dquote.replace_all(&out, "`").into_owned();
        out = self
            .re_bigserial
            .replace_all(&out, "BIGINT AUTO_INCREMENT")
            .into_owned();
        out = self
            .re_serial
            .replace_all(&out, "INT AUTO_INCREMENT")
            .into_owned();
        out = self.re_boolean.replace_all(&out, "TINYINT(1)").into_owned();
        out = self.re_integer.replace_all(&out, "INT").into_owned();
        out = self
            .re_double_precision
            .replace_all(&out, "DOUBLE")
            .into_owned();
        out = self.re_real.replace_all(&out, "FLOAT").into_owned();
        out = self.re_timestamp.replace_all(&out, "DATETIME").into_owned();
        out = self.re_cts.replace_all(&out, "NOW()").into_owned();
        out = self.re_coalesce.replace_all(&out, "IFNULL(").into_owned();
        out = self
            .re_to_timestamp
            .replace_all(&out, "FROM_UNIXTIME(")
            .into_owned();
        out = self
            .re_limit_offset
            .replace_all(&out, |caps: &regex::Captures| {
                let cnt = &caps[1];
                let off = &caps[2];
                format!("LIMIT {}, {}", off, cnt)
            })
            .into_owned();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证分页转换
    #[test]
    fn test_limit_conversion() {
        let src = "select * from t limit 5, 10";
        let out = convert_sql(src, "mysql", "postgresql").unwrap();
        assert!(out.to_lowercase().contains("limit 10 offset 5"));
    }

    /// 验证自增主键与表选项移除
    #[test]
    fn test_create_table_auto_inc() {
        let src = "CREATE TABLE `users` ( `id` int AUTO_INCREMENT PRIMARY KEY ) ENGINE=InnoDB DEFAULT CHARSET=utf8";
        let out = convert_sql(src, "mysql", "postgresql").unwrap();
        assert!(out.contains("\"users\""));
        assert!(out.contains("SERIAL PRIMARY KEY"));
        assert!(!out.to_lowercase().contains("engine="));
        assert!(!out.to_lowercase().contains("charset"));
    }

    /// 验证数据类型与常用函数映射
    #[test]
    fn test_datatypes_functions() {
        let src = "CREATE TABLE t ( f1 tinyint(1), f2 datetime, f3 double, f4 float, f5 mediumint, f6 tinyint, f7 int unsigned ); select now(), ifnull(a,0), from_unixtime(ts) from t limit 0, 1";
        let out = convert_sql(src, "mysql", "postgresql").unwrap();
        println!("{}", out);
        let l = out.to_lowercase();
        assert!(l.contains("boolean"));
        assert!(l.contains("timestamp"));
        assert!(l.contains("double precision"));
        assert!(l.contains("real"));
        assert!(l.contains("integer"));
        assert!(l.contains("smallint"));
        assert!(l.contains("current_timestamp"));
        assert!(l.contains("coalesce("));
        assert!(l.contains("to_timestamp("));
        assert!(l.contains("limit 1 offset 0"));
        assert!(!l.contains("unsigned"));
    }

    #[test]
    fn test_pg_limit_conversion() {
        let src = "select * from t limit 10 offset 5";
        let out = convert_sql(src, "postgresql", "mysql").unwrap();
        assert!(out.to_lowercase().contains("limit 5, 10"));
    }

    #[test]
    fn test_pg_datatypes_functions() {
        let src = "CREATE TABLE t ( f1 boolean, f2 timestamp, f3 double precision, f4 real, f5 integer, f6 bigserial, f7 serial ); select current_timestamp, coalesce(a,0), to_timestamp(ts) from t limit 1 offset 0";
        let out = convert_sql(src, "postgresql", "mysql").unwrap();
        let l = out.to_lowercase();
        assert!(l.contains("tinyint(1)"));
        assert!(l.contains("datetime"));
        assert!(l.contains("double"));
        assert!(l.contains("float"));
        assert!(l.contains("int"));
        assert!(l.contains("bigint auto_increment"));
        assert!(l.contains("int auto_increment"));
        assert!(l.contains("now()"));
        assert!(l.contains("ifnull("));
        assert!(l.contains("from_unixtime("));
        assert!(l.contains("limit 0, 1"));
    }
}
