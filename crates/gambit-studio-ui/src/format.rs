//! Formatting helpers.

/// Format an integer with thousands separators.
pub fn format_num(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let formatted: String = out.chars().rev().collect();
    if n < 0 {
        format!("-{formatted}")
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_num_commas() {
        assert_eq!(format_num(1_234_567), "1,234,567");
        assert_eq!(format_num(0), "0");
    }
}
