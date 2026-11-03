//! Classifies the CLI's output lines: `==> text` marks an install step.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    Step(String),
    Log(String),
}

pub fn classify(line: &str) -> Line {
    match line.strip_prefix("==> ") {
        Some(step) if !step.trim().is_empty() => Line::Step(step.to_owned()),
        _ => Line::Log(line.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_lines_are_steps() {
        assert_eq!(
            classify("==> install base system"),
            Line::Step("install base system".into())
        );
        assert_eq!(
            classify("pacstrap: downloading"),
            Line::Log("pacstrap: downloading".into())
        );
        assert_eq!(classify("==>"), Line::Log("==>".into()));
        assert_eq!(classify("==> "), Line::Log("==> ".into()));
    }
}
