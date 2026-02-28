use anyhow::{Result, anyhow};
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 轻量级模板引擎：
/// 解析 HTML 文本，处理形如 `<rsp:include page="header" />` 的指令，
/// 将 `header.html` 的内容嵌入到该位置。
/// - `base_dir`：静态文件根目录，例如 `./static`
/// - `file_rel`：相对路径，例如 `index.html`
pub fn render_with_includes(base_dir: &str, file_rel: &str) -> Result<String> {
    let base = canonicalize_dir(base_dir)?;
    let mut visited = HashSet::new();
    let content = render_recursive(&base, Path::new(file_rel), 0, &mut visited)?;
    Ok(content)
}

fn canonicalize_dir(dir: &str) -> Result<PathBuf> {
    let p = PathBuf::from(dir);
    let c = p
        .canonicalize()
        .map_err(|e| anyhow!("canonicalize base dir failed: {}", e))?;
    Ok(c)
}

fn safe_join(base: &Path, rel: &Path) -> Result<PathBuf> {
    let mut p = base.to_path_buf();
    p.push(rel);
    let c = p
        .canonicalize()
        .map_err(|e| anyhow!("canonicalize join failed: {}", e))?;
    if !c.starts_with(base) {
        return Err(anyhow!("include path escapes base directory"));
    }
    Ok(c)
}

fn render_recursive(
    base: &Path,
    file_rel: &Path,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
) -> Result<String> {
    if depth > 8 {
        // 防止无限递归
        return Err(anyhow!("max include depth exceeded"));
    }
    let full = safe_join(base, file_rel)?;
    if visited.contains(&full) {
        // 检测到循环包含，跳过该文件内容
        return Ok(String::new());
    }
    let raw = fs::read_to_string(&full)
        .map_err(|e| anyhow!("read file failed ({}): {}", full.display(), e))?;

    // 记录已访问，粗略避免环
    visited.insert(full.clone());

    // 匹配 <rsp:include page="..." /> 或 <rsp:include page='...' />（自闭合）
    // 注意：Rust regex 不支持反向引用，使用双/单引号的交替分组匹配
    // 捕获组1为双引号内容，组2为单引号内容（二选一）
    let re = Regex::new(r#"<\s*rsp:include\s+page\s*=\s*(?:\"([^\"]+)\"|'([^']+)')\s*/\s*>"#)
        .expect("regex compile");

    let mut out = String::new();
    let mut last_idx = 0;
    for m in re.find_iter(&raw) {
        out.push_str(&raw[last_idx..m.start()]);
        // 提取 page 值（支持双引号或单引号）
        let caps = re
            .captures(m.as_str())
            .ok_or_else(|| anyhow!("capture failed"))?;
        let page = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|s| s.as_str())
            .unwrap_or("");
        let include_rel = if page.ends_with(".html") {
            PathBuf::from(page)
        } else {
            PathBuf::from(format!("{}.html", page))
        };
        let included = match render_recursive(base, &include_rel, depth + 1, visited) {
            Ok(s) => s,
            Err(_) => String::new(), // 若包含失败，插入空字符串以保证页面可用
        };
        out.push_str(&included);
        last_idx = m.end();
    }
    out.push_str(&raw[last_idx..]);
    Ok(out)
}
