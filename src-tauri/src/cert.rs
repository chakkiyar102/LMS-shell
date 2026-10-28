//! Certificate as a one-page PDF, for Android where the webview cannot print. Uses the PDF base-14
//! fonts, so nothing is embedded and the file stays a few KB.
//! base-14 fonts are Latin-1 only; other scripts print as '?'. Embed a TTF if that matters.

/// Approximate advance width (in em) for centring; base-14 metrics vary per glyph.
/// average widths, so centring can be off by a few points on unusual names.
fn width(text: &str, size: f64, em: f64) -> f64 { text.chars().count() as f64 * size * em }

fn latin1(text: &str) -> String {
    text.chars().map(|c| match c {
        '(' | ')' | '\\' => format!("\\{c}"),
        c if (c as u32) < 256 => c.to_string(),
        '\u{2013}' | '\u{2014}' => "-".into(),
        _ => "?".into(),
    }).collect()
}

pub fn pdf(name: &str, course: &str, issued_on: &str, id: &str) -> Vec<u8> {
    const W: f64 = 842.0; // A4 landscape
    const H: f64 = 595.0;
    // (font resource, size, average em width, baseline y, text)
    let lines = [
        ("F3", 12.0, 0.62, 470.0, shared::brand::APP_NAME.to_uppercase()),
        ("F1", 34.0, 0.455, 420.0, "Certificate of Completion".to_string()),
        ("F3", 14.0, 0.5, 365.0, "This certifies that".to_string()),
        ("F2", 40.0, 0.45, 310.0, name.to_string()),
        ("F3", 14.0, 0.5, 262.0, "has completed".to_string()),
        ("F4", 20.0, 0.535, 228.0, course.to_string()),
        ("F3", 11.0, 0.485, 130.0, format!("Issued {issued_on}   |   Certificate {id}")),
    ];
    let mut stream = format!("0.83 0.66 0.26 RG 3 w 36 36 {} {} re S\n0.1 0.1 0.1 RG 1 w 46 46 {} {} re S\n",
        W - 72.0, H - 72.0, W - 92.0, H - 92.0);
    for (font, size, em, y, text) in &lines {
        let t = latin1(text);
        let x = ((W - width(text, *size, *em)) / 2.0).max(50.0);
        stream += &format!("BT /{font} {size} Tf {x:.1} {y} Td ({t}) Tj ET\n");
    }
    let fonts = [("F1", "Times-Bold"), ("F2", "Times-BoldItalic"), ("F3", "Helvetica"), ("F4", "Helvetica-Bold")];
    let mut objs = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W} {H}] /Contents 4 0 R /Resources << /Font << {} >> >> >>",
            fonts.iter().enumerate().map(|(i, (k, _))| format!("/{k} {} 0 R", 5 + i)).collect::<Vec<_>>().join(" ")),
        format!("<< /Length {} >>\nstream\n{stream}endstream", stream.chars().count()),
    ];
    objs.extend(fonts.iter().map(|(_, f)| format!("<< /Type /Font /Subtype /Type1 /BaseFont /{f} /Encoding /WinAnsiEncoding >>")));

    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![];
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).bytes());
        // WinAnsi text: write Latin-1 bytes, not UTF-8
        out.extend(o.chars().map(|c| c as u32 as u8));
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for off in offsets { out.extend(format!("{off:010} 00000 n \n").bytes()); }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).bytes());
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn pdf_is_well_formed() {
        let p = super::pdf("Riley (QA) Brooks", "Getting started with Brightline Notes", "2026-10-15", "CERT-1");
        let s = String::from_utf8_lossy(&p);
        assert!(s.starts_with("%PDF-1.4") && s.ends_with("%%EOF\n"));
        assert!(s.contains("(Riley \\(QA\\) Brooks)"));
        // startxref points at the xref table
        let at: usize = s.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(s[at..].starts_with("xref"));
    }
}
