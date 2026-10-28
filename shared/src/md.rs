//! A small, safe subset of Markdown for model replies (and our own short strings): **bold**, *italic* /
//! _italic_, `code`, paragraphs, line breaks, bullet and numbered lists, and #..### headings (shown as bold
//! lines). It only ever produces this tree; the UI turns it into elements, never into HTML text, so model
//! output can't inject markup. Links become plain "label (url)". Unmatched markers stay literal, and a
//! backslash keeps the next marker literal.

/// Told to every model whose reply the learner reads, so the reply suits this renderer.
pub const REPLY_FORMAT: &str = "Formatting: **bold**, *italic*, `code`, and short \"- \" bullet or \"1. \" numbered \
    lists are shown formatted. Don't use tables, headings or links.";

#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Code(String),
    Break,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Para(Vec<Inline>),
    /// A # .. ### heading, shown as a bold line.
    Heading(Vec<Inline>),
    Bullets(Vec<Vec<Inline>>),
    /// The first item's number, and the items.
    Numbered(u32, Vec<Vec<Inline>>),
    /// A ``` fenced block, verbatim.
    Code(String),
}

const ESCAPABLE: &[char] = &['*', '_', '`', '\\', '#', '[', ']', '-'];

/// Inline formatting in one run of text; newlines become `Break`.
pub fn inline(s: &str) -> Vec<Inline> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    parse(&c, &mut out);
    merge(out)
}

fn push_text(out: &mut Vec<Inline>, t: &str) {
    if let Some(Inline::Text(last)) = out.last_mut() { last.push_str(t) } else if !t.is_empty() { out.push(Inline::Text(t.into())) }
}

fn merge(v: Vec<Inline>) -> Vec<Inline> {
    let mut out = Vec::new();
    for i in v { match i { Inline::Text(t) => push_text(&mut out, &t), x => out.push(x) } }
    out
}

/// The index just past a code span opening at `i`, if it closes.
fn code_end(c: &[char], i: usize) -> Option<usize> {
    (i + 1..c.len()).find(|&j| c[j] == '`').map(|j| j + 1)
}

/// Where the closing `m` (1 or 2 chars of `ch`) for an opener ending at `from` is: the closer must follow a
/// non-space, and code spans and escapes in between are skipped. In a run of markers the last ones close.
fn find_close(c: &[char], from: usize, ch: char, n: usize) -> Option<usize> {
    let mut j = from;
    while j < c.len() {
        match c[j] {
            '\\' => { j += 2; continue }
            '`' => { if let Some(e) = code_end(c, j) { j = e; continue } }
            x if x == ch && j > from && !c[j - 1].is_whitespace() && c[j..].len() >= n && c[j..j + n].iter().all(|&y| y == ch) => {
                // a run longer than the marker: close with its last n chars (***x*** is bold around italic)
                let mut k = j;
                while k + n < c.len() && c[k + n] == ch { k += 1 }
                if ch == '_' && c.get(k + n).is_some_and(|x| x.is_alphanumeric()) { j = k + n; continue }
                return Some(k);
            }
            _ => {}
        }
        j += 1;
    }
    None
}

fn parse(c: &[char], out: &mut Vec<Inline>) {
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        match ch {
            '\\' if c.get(i + 1).is_some_and(|x| ESCAPABLE.contains(x)) => { push_text(out, &c[i + 1].to_string()); i += 2; }
            '\n' => { out.push(Inline::Break); i += 1; }
            '`' => match code_end(c, i) {
                Some(e) if e > i + 2 => { out.push(Inline::Code(c[i + 1..e - 1].iter().collect())); i = e; }
                _ => { push_text(out, "`"); i += 1; }
            },
            '*' | '_' => {
                let n = if c.get(i + 1) == Some(&ch) { 2 } else { 1 };
                let opens = c.get(i + n).is_some_and(|x| !x.is_whitespace())
                    && !(ch == '_' && i > 0 && c[i - 1].is_alphanumeric());
                let close = if opens { find_close(c, i + n, ch, n) } else { None };
                match close {
                    Some(k) => {
                        let mut inner = Vec::new();
                        parse(&c[i + n..k], &mut inner);
                        out.push(if n == 2 { Inline::Bold(merge(inner)) } else { Inline::Italic(merge(inner)) });
                        i = k + n;
                    }
                    None => {
                        // a lone ** may still open an italic: *x*
                        push_text(out, &c[i..i + n].iter().collect::<String>());
                        i += n;
                    }
                }
            }
            '[' => {
                // [label](url) → label (url), as text: never a link
                let close = (i + 1..c.len()).find(|&j| c[j] == ']');
                let url = close.filter(|&j| c.get(j + 1) == Some(&'(')).and_then(|j| (j + 2..c.len()).find(|&k| c[k] == ')').map(|k| (j, k)));
                match url {
                    Some((j, k)) => {
                        parse(&c[i + 1..j], out);
                        push_text(out, &format!(" ({})", c[j + 2..k].iter().collect::<String>()));
                        i = k + 1;
                    }
                    None => { push_text(out, "["); i += 1; }
                }
            }
            _ => { push_text(out, &ch.to_string()); i += 1; }
        }
    }
}

fn bullet(l: &str) -> Option<&str> {
    let t = l.trim_start();
    ["- ", "* ", "+ ", "• "].iter().find_map(|m| t.strip_prefix(m))
}

fn numbered(l: &str) -> Option<(u32, &str)> {
    let t = l.trim_start();
    let d = t.find(|ch: char| !ch.is_ascii_digit())?;
    if d == 0 || d > 4 { return None; }
    let rest = t[d..].strip_prefix(". ").or_else(|| t[d..].strip_prefix(") "))?;
    Some((t[..d].parse().ok()?, rest))
}

fn heading(l: &str) -> Option<&str> {
    let t = l.trim_start();
    let h = t.find(|ch: char| ch != '#')?;
    (1..=6).contains(&h).then(|| t[h..].strip_prefix(' ')).flatten()
}

/// The whole reply: paragraphs (blank lines), lists, headings and fenced code; single newlines are line breaks.
pub fn blocks(s: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, out: &mut Vec<Block>| {
        if !para.is_empty() { out.push(Block::Para(inline(&para.join("\n")))); para.clear(); }
    };
    let mut lines = s.lines().peekable();
    // whether the last block is a list still open to continuation lines
    let mut in_list = false;
    while let Some(l) = lines.next() {
        if l.trim_start().starts_with("```") {
            flush(&mut para, &mut out);
            let mut code = Vec::new();
            for m in lines.by_ref() { if m.trim_start().starts_with("```") { break } code.push(m) }
            out.push(Block::Code(code.join("\n")));
            in_list = false;
            continue;
        }
        if l.trim().is_empty() { flush(&mut para, &mut out); in_list = false; continue; }
        if let Some(h) = heading(l) { flush(&mut para, &mut out); out.push(Block::Heading(inline(h.trim()))); in_list = false; continue; }
        if let Some(b) = bullet(l) {
            flush(&mut para, &mut out);
            match out.last_mut() { Some(Block::Bullets(v)) if in_list => v.push(inline(b)), _ => out.push(Block::Bullets(vec![inline(b)])) }
            in_list = true;
            continue;
        }
        if let Some((n, b)) = numbered(l) {
            flush(&mut para, &mut out);
            match out.last_mut() { Some(Block::Numbered(_, v)) if in_list => v.push(inline(b)), _ => out.push(Block::Numbered(n, vec![inline(b)])) }
            in_list = true;
            continue;
        }
        if in_list {
            // a wrapped list item: continues the last one on a new line
            if let Some(Block::Bullets(v) | Block::Numbered(_, v)) = out.last_mut() {
                if let Some(item) = v.last_mut() { item.push(Inline::Break); item.extend(inline(l.trim())); *item = merge(std::mem::take(item)); }
            }
            continue;
        }
        para.push(l);
    }
    flush(&mut para, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use Inline::*;
    fn t(s: &str) -> Inline { Text(s.into()) }

    #[test]
    fn the_reported_reply() {
        assert_eq!(inline("what do you think a **process** is, and what is the **item**"),
            vec![t("what do you think a "), Bold(vec![t("process")]), t(" is, and what is the "), Bold(vec![t("item")])]);
    }

    #[test]
    fn italics_code_and_nesting() {
        assert_eq!(inline("*one* and _two_"), vec![Italic(vec![t("one")]), t(" and "), Italic(vec![t("two")])]);
        assert_eq!(inline("**bold *and italic***"), vec![Bold(vec![t("bold "), Italic(vec![t("and italic")])])]);
        assert_eq!(inline("***both***"), vec![Bold(vec![Italic(vec![t("both")])])]);
        assert_eq!(inline("use `a**b**c` here"), vec![t("use "), Code("a**b**c".into()), t(" here")]);
        assert_eq!(inline("**see `x**y`**"), vec![Bold(vec![t("see "), Code("x**y".into())])]);
    }

    #[test]
    fn unmatched_and_escaped_markers_stay_literal() {
        assert_eq!(inline("2 * 3 = 6"), vec![t("2 * 3 = 6")]);
        assert_eq!(inline("**open but never closed"), vec![t("**open but never closed")]);
        assert_eq!(inline("a ** b ** c"), vec![t("a ** b ** c")]);
        assert_eq!(inline(r"\*not italic\*"), vec![t("*not italic*")]);
        assert_eq!(inline(r"\*\*not bold\*\*"), vec![t("**not bold**")]);
        assert_eq!(inline("snake_case_name stays"), vec![t("snake_case_name stays")]);
        assert_eq!(inline("a `lone backtick"), vec![t("a `lone backtick")]);
        assert_eq!(inline("**"), vec![t("**")]);
        assert_eq!(inline("*"), vec![t("*")]);
    }

    #[test]
    fn links_are_text_and_newlines_break() {
        assert_eq!(inline("see [the help](javascript:alert(1)) now"), vec![t("see the help (javascript:alert(1)) now")]);
        assert_eq!(inline("[just brackets]"), vec![t("[just brackets]")]);
        assert_eq!(inline("one\ntwo"), vec![t("one"), Break, t("two")]);
        // markup in model text is text, nothing more
        assert_eq!(inline("<img src=x onerror=alert(1)>"), vec![t("<img src=x onerror=alert(1)>")]);
    }

    #[test]
    fn blocks_paragraphs_lists_headings() {
        let b = blocks("## Next step\nOpen the **form**.\nThen save.\n\n- Add *Quantity*\n- Add Unit price\n  with two decimals\n\n3. Third\n4. Fourth\n\n```\nlet x = **1**;\n```");
        assert_eq!(b, vec![
            Block::Heading(vec![t("Next step")]),
            Block::Para(vec![t("Open the "), Bold(vec![t("form")]), t("."), Break, t("Then save.")]),
            Block::Bullets(vec![vec![t("Add "), Italic(vec![t("Quantity")])], vec![t("Add Unit price"), Break, t("with two decimals")]]),
            Block::Numbered(3, vec![vec![t("Third")], vec![t("Fourth")]]),
            Block::Code("let x = **1**;".into()),
        ]);
        // a line starting "*word*" is italic, not a bullet; "1.5 hours" is not a list
        assert_eq!(blocks("*Tip*: save often"), vec![Block::Para(vec![Italic(vec![t("Tip")]), t(": save often")])]);
        assert_eq!(blocks("1.5 hours"), vec![Block::Para(vec![t("1.5 hours")])]);
        assert_eq!(blocks(""), vec![]);
    }
}
