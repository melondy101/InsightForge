//! 分块纯函数（02 号票 · spec §索引与检索）：
//! Markdown 优先按标题切分，超长小节再按段落与长度窗口打包。
//!
//! 无损约定：非空白输入的分块按顺序以 `\n` 连接后还原原文（按行覆盖，
//! 每一行恰好属于一个块）；行尾换行符与 CRLF 不保留在块文本里。
//! 单行超过窗口时按字符窗口硬切（同一段切块的文本直接连接可还原该行）。

/// 默认单块字符窗口上限。
pub const DEFAULT_MAX_CHUNK_CHARS: usize = 1200;

/// 分块配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkConfig {
    /// 单块文本的最大字符数（按 Unicode 标量计）。
    pub max_chars: usize,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            max_chars: DEFAULT_MAX_CHUNK_CHARS,
        }
    }
}

/// 一个检索单元：文件内行号定位（1 起闭区间）、标题路径与文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// 所属标题链（如 `"甲 > 乙"`）；无标题时为空串。
    pub heading_path: String,
    /// 起始行（1 起，闭区间）。
    pub start_line: usize,
    /// 结束行（闭区间）。
    pub end_line: usize,
    /// 块文本（各原始行以 `\n` 连接）。
    pub text: String,
}

/// Markdown 分块：优先按标题切分小节，小节超过窗口时按段落与长度窗口打包。
pub fn chunk_markdown(text: &str, config: ChunkConfig) -> Vec<Chunk> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let max = config.max_chars.max(1);
    let mut chunks = Vec::new();
    for section in split_sections(&lines) {
        chunks.extend(pack_section(
            &lines,
            section.start,
            section.end,
            &section.heading_path,
            max,
        ));
    }
    chunks
}

/// 纯文本分块：无标题概念，全文按段落与长度窗口打包。
pub fn chunk_plain(text: &str, config: ChunkConfig) -> Vec<Chunk> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = text.lines().collect();
    let max = config.max_chars.max(1);
    pack_section(&lines, 0, lines.len() - 1, "", max)
}

/// 一个标题小节：行区间与所属标题链。
struct Section {
    start: usize,
    end: usize,
    heading_path: String,
}

/// 识别 ATX 标题行（1–6 个 `#`，后接空格/制表符或行尾），返回 (层级, 标题文本)。
fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    Some((level, rest.trim()))
}

/// 把行列表按标题切成小节：标题行开启新小节（任意层级），
/// 首个标题之前的行是前导小节（heading_path 为空）。
fn split_sections(lines: &[&str]) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut stack: [Option<&str>; 6] = Default::default();
    let heading_lines: Vec<usize> = (0..lines.len())
        .filter(|&i| parse_heading(lines[i]).is_some())
        .collect();

    match heading_lines.first() {
        Some(&first) if first > 0 => sections.push(Section {
            start: 0,
            end: first - 1,
            heading_path: String::new(),
        }),
        None if !lines.is_empty() => sections.push(Section {
            start: 0,
            end: lines.len() - 1,
            heading_path: String::new(),
        }),
        _ => {}
    }

    for (index, &heading) in heading_lines.iter().enumerate() {
        let (level, title) = parse_heading(lines[heading]).expect("already validated heading");
        stack[level - 1] = Some(title);
        for slot in stack.iter_mut().skip(level) {
            *slot = None;
        }
        let end = heading_lines
            .get(index + 1)
            .map_or(lines.len() - 1, |&next| next - 1);
        let heading_path = stack
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
            .join(" > ");
        sections.push(Section {
            start: heading,
            end,
            heading_path,
        });
    }
    sections
}

/// 把小节切成「段落单元」：每个非空行块连同其后紧跟的空行组成一个单元；
/// 小节开头的空行并入其后首个非空块；全空小节自成一个单元。
fn build_units(lines: &[&str], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut units = Vec::new();
    let mut i = start;
    while i <= end {
        let unit_start = i;
        while i <= end && lines[i].trim().is_empty() {
            i += 1;
        }
        if i > end {
            units.push((unit_start, end));
            break;
        }
        while i <= end && !lines[i].trim().is_empty() {
            i += 1;
        }
        while i <= end && lines[i].trim().is_empty() {
            i += 1;
        }
        units.push((unit_start, i - 1));
    }
    units
}

/// 把小节按段落与长度窗口打包成块。
///
/// 贪心装填：加入下一单元会超窗时先收束当前块；单个单元自身超窗时
/// 按行打包，单行超限再按字符窗口硬切。
fn pack_section(
    lines: &[&str],
    start: usize,
    end: usize,
    heading_path: &str,
    max: usize,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let units = build_units(lines, start, end);
    let mut current: Vec<usize> = Vec::new();
    let mut current_chars = 0usize;

    for &(unit_start, unit_end) in &units {
        let unit_chars = lines[unit_start..=unit_end].join("\n").chars().count();
        if !current.is_empty() && current_chars + 1 + unit_chars > max {
            push_chunk(&mut chunks, lines, &current, heading_path);
            current.clear();
            current_chars = 0;
        }
        if unit_chars > max {
            chunks.extend(split_oversize_unit(
                lines,
                unit_start,
                unit_end,
                heading_path,
                max,
            ));
        } else {
            current.extend(unit_start..=unit_end);
            current_chars = if current_chars == 0 {
                unit_chars
            } else {
                current_chars + 1 + unit_chars
            };
        }
    }
    if !current.is_empty() {
        push_chunk(&mut chunks, lines, &current, heading_path);
    }
    chunks
}

/// 用给定行区间构造一个块（行号转 1 起）。
fn push_chunk(chunks: &mut Vec<Chunk>, lines: &[&str], indices: &[usize], heading_path: &str) {
    let first = indices.first().copied().expect("non-empty line indices");
    let last = indices.last().copied().expect("non-empty line indices");
    chunks.push(Chunk {
        heading_path: heading_path.to_string(),
        start_line: first + 1,
        end_line: last + 1,
        text: indices
            .iter()
            .map(|&i| lines[i])
            .collect::<Vec<_>>()
            .join("\n"),
    });
}

/// 超窗单元的硬切：先按行贪心打包，单行超限再按字符窗口切分
/// （窗口块的行号均为该行自身，文本直接连接可还原该行）。
fn split_oversize_unit(
    lines: &[&str],
    unit_start: usize,
    unit_end: usize,
    heading_path: &str,
    max: usize,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_chars = 0usize;

    for index in unit_start..=unit_end {
        let line = lines[index];
        let line_chars = line.chars().count();
        if line_chars > max {
            if !current.is_empty() {
                push_chunk(&mut chunks, lines, &current, heading_path);
                current.clear();
                current_chars = 0;
            }
            let chars: Vec<char> = line.chars().collect();
            for window in chars.chunks(max) {
                let text: String = window.iter().collect();
                chunks.push(Chunk {
                    heading_path: heading_path.to_string(),
                    start_line: index + 1,
                    end_line: index + 1,
                    text,
                });
            }
            continue;
        }
        if !current.is_empty() && current_chars + 1 + line_chars > max {
            push_chunk(&mut chunks, lines, &current, heading_path);
            current.clear();
            current_chars = 0;
        }
        current.push(index);
        current_chars = if current_chars == 0 {
            line_chars
        } else {
            current_chars + 1 + line_chars
        };
    }
    if !current.is_empty() {
        push_chunk(&mut chunks, lines, &current, heading_path);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(chunks: &[Chunk]) -> Vec<&str> {
        chunks.iter().map(|c| c.text.as_str()).collect()
    }

    #[test]
    fn headings_take_priority_over_length_windows() {
        let text = "# 甲\n\n第一段。\n\n# 乙\n\n第二段。\n";
        let chunks = chunk_markdown(text, ChunkConfig { max_chars: 1000 });

        assert_eq!(chunks.len(), 2, "两个 H1 小节必须各自成块");
        assert_eq!(chunks[0].heading_path, "甲");
        assert_eq!(chunks[1].heading_path, "乙");
        let texts = texts(&chunks);
        assert!(texts[0].starts_with("# 甲"), "块文本必须携带标题行");
        assert!(texts[1].starts_with("# 乙"));
    }

    #[test]
    fn nested_headings_form_ancestor_paths() {
        let text = "# 甲\n\n## 乙\n\n正文。\n\n# 丙\n";
        let chunks = chunk_markdown(text, ChunkConfig { max_chars: 1000 });

        let paths: Vec<&str> = chunks.iter().map(|c| c.heading_path.as_str()).collect();
        assert_eq!(paths, vec!["甲", "甲 > 乙", "丙"], "标题路径必须是祖先链");
    }

    #[test]
    fn sections_exceeding_window_are_packed_by_paragraphs() {
        let paragraph = "字".repeat(50);
        let mut lines: Vec<String> = Vec::new();
        for i in 0..6 {
            if i > 0 {
                lines.push(String::new());
            }
            lines.push(paragraph.clone());
        }
        let text = lines.join("\n");
        let chunks = chunk_markdown(&text, ChunkConfig { max_chars: 120 });

        assert_eq!(chunks.len(), 3, "6 段 × 51 字符、窗口 120 → 3 块");
        for chunk in &chunks {
            assert!(
                chunk.text.chars().count() <= 120,
                "每块不得超过窗口，got {}",
                chunk.text.chars().count()
            );
            assert_eq!(
                chunk.text.matches(paragraph.as_str()).count(),
                2,
                "段落不得被拆开（窗口内整段保留）"
            );
        }
        assert_eq!(chunks[1].start_line, 5, "块边界必须落在段落边界");
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[2].end_line, 11);
    }

    #[test]
    fn oversize_single_line_is_hard_split_by_length_window() {
        let line = "长".repeat(400);
        let chunks = chunk_markdown(&line, ChunkConfig { max_chars: 160 });

        assert_eq!(chunks.len(), 3, "400 字符单行、窗口 160 → 3 窗");
        for chunk in &chunks {
            assert_eq!(chunk.start_line, 1);
            assert_eq!(chunk.end_line, 1);
            assert!(chunk.text.chars().count() <= 160);
        }
        let rejoined: String = chunks.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(rejoined, line, "同一行的各窗直接连接必须还原该行");
    }

    #[test]
    fn plain_text_chunks_have_no_heading_paths() {
        let text = "# 这不是标题\n\n段落一。\n\n段落二。\n";
        let chunks = chunk_plain(text, ChunkConfig { max_chars: 1000 });

        assert_eq!(chunks.len(), 1, "纯文本不做标题切分");
        assert_eq!(chunks[0].heading_path, "");
        assert_eq!(chunks[0].text, "# 这不是标题\n\n段落一。\n\n段落二。");
    }

    #[test]
    fn empty_and_blank_inputs_yield_no_chunks() {
        let config = ChunkConfig::default();
        assert!(chunk_markdown("", config).is_empty());
        assert!(chunk_markdown("\n\n  \n", config).is_empty());
        assert!(chunk_plain("", config).is_empty());
    }

    #[test]
    fn property_chunks_cover_every_line_within_window() {
        // 确定性伪随机（xorshift 常数 LCG），避免引入属性测试依赖。
        let mut state: u64 = 0x02_5F_02;
        let mut lcg = |state: &mut u64| {
            *state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *state
        };
        let max = 200usize;
        for _case in 0..300 {
            let mut lines: Vec<String> = Vec::new();
            let sections = 1 + (lcg(&mut state) % 4) as usize;
            for s in 0..sections {
                if s > 0 || lcg(&mut state) % 2 == 0 {
                    let level = 1 + (lcg(&mut state) % 3) as usize;
                    lines.push(format!("{} 标题{s}", "#".repeat(level)));
                }
                let paragraphs = 1 + (lcg(&mut state) % 3) as usize;
                for _p in 0..paragraphs {
                    let n_lines = 1 + (lcg(&mut state) % 3) as usize;
                    for _l in 0..n_lines {
                        let chars = 10 + (lcg(&mut state) % 60) as usize;
                        lines.push("字".repeat(chars));
                    }
                    if lcg(&mut state) % 3 == 0 {
                        lines.push(String::new());
                    }
                }
                if lcg(&mut state) % 2 == 0 {
                    lines.push(String::new());
                }
            }
            // 块文本不保留文末换行（模块文档约定）：属性测试的输入去掉尾部空行，
            // 使 lines() 与生成行一一对应；行内/段间空行结构仍被覆盖。
            while lines.last().is_some_and(String::is_empty) {
                lines.pop();
            }
            let doc = lines.join("\n");
            let chunks = chunk_markdown(&doc, ChunkConfig { max_chars: max });
            assert!(!chunks.is_empty(), "有内容的文档至少产生一个块");

            let mut expected_start = 1usize;
            for chunk in &chunks {
                assert!(
                    chunk.text.chars().count() <= max,
                    "每块都不得超过窗口（生成器保证单行不超窗）"
                );
                assert_eq!(
                    chunk.start_line, expected_start,
                    "块的行区间必须首尾相接覆盖全文"
                );
                expected_start = chunk.end_line + 1;
            }
            assert_eq!(
                expected_start,
                doc.lines().count() + 1,
                "最后一个块必须覆盖到文末"
            );
            let covered: String = chunks
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(covered, doc, "分块必须无损：按行覆盖原文");
        }
    }
}
