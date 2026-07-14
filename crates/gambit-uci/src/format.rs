//! UCI output formatting.

use crate::parse::Info;
use gambit_analysis::{Analysis, Score};

/// Format an `info` line from search output.
pub fn format_info(analysis: &Analysis) -> String {
    let mut info = Info {
        depth: Some(analysis.depth),
        nodes: Some(analysis.nodes),
        time_ms: Some(analysis.time_ms),
        pv: analysis.pv.clone(),
        ..Info::default()
    };
    match analysis.score {
        Score::Cp(cp) => info.score_cp = Some(cp),
        Score::Mate(m) => info.score_mate = Some(m),
    }
    format_info_struct(&info)
}

fn format_info_struct(info: &Info) -> String {
    let mut parts = vec!["info".to_string()];
    if let Some(d) = info.depth {
        parts.push(format!("depth {d}"));
    }
    if let Some(cp) = info.score_cp {
        parts.push(format!("score cp {cp}"));
    } else if let Some(m) = info.score_mate {
        parts.push(format!("score mate {m}"));
    }
    if let Some(n) = info.nodes {
        parts.push(format!("nodes {n}"));
    }
    if let Some(t) = info.time_ms {
        parts.push(format!("time {t}"));
        if nps(info.nodes, t) > 0 {
            parts.push(format!("nps {}", nps(info.nodes, t)));
        }
    }
    if !info.pv.is_empty() {
        parts.push("pv".to_string());
        for m in &info.pv {
            parts.push(m.to_uci());
        }
    }
    parts.join(" ")
}

fn nps(nodes: Option<u64>, time_ms: u64) -> u64 {
    nodes.map_or(0, |n| {
        if time_ms == 0 {
            0
        } else {
            n.saturating_mul(1000) / time_ms
        }
    })
}

/// Format a `bestmove` line.
pub fn format_bestmove(uci: &str) -> String {
    format!("bestmove {uci}")
}
