//! Compose world instructions from the actual XR mode, without duplicate actions.
pub(super) fn text<'a>(status: &'a str, instruction: &'a str, navigation: &'a str) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    for part in [status, instruction, navigation].map(str::trim) {
        if !part.is_empty() && !parts.iter().any(|old| *old == part || old.lines().any(|line| line.trim() == part)) {
            parts.push(part);
        }
    }
    parts.join("\n")
}

pub(super) fn override_hint(status: &mut String, instruction: &mut String, hint: &str) {
    let previous = instruction.trim();
    *status = status.lines().filter(|line| previous.is_empty() || line.trim() != previous).collect::<Vec<_>>().join("\n");
    *instruction = hint.trim().to_owned();
    if !instruction.is_empty() && !status.lines().any(|line| line.trim() == instruction.as_str()) {
        *status = if status.is_empty() { instruction.clone() } else { format!("{instruction}\n{status}") };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn world_hud_preserves_status_navigation_and_one_copy_of_each_instruction() {
        assert_eq!(text("  Distance: 2 m\nTrigger confirms. ", "Trigger confirms.", "Both grips navigate."),
            "Distance: 2 m\nTrigger confirms.\nBoth grips navigate.");
        assert_eq!(text(" ", "Trigger selects.", "Trigger selects."), "Trigger selects.");
        assert_eq!(text("Ready", "", ""), "Ready");
    }

    #[test]
    fn world_hud_target_override_replaces_old_actions_without_losing_errors() {
        let mut status = "Hold trigger to select.\nUnlock the object first.".to_owned();
        let mut instruction = "Hold trigger to select.".to_owned();
        override_hint(&mut status, &mut instruction, "Trigger edits this wood joint.");
        let output = text(&status, &instruction, "Both grips navigate.");
        assert_eq!(output, "Trigger edits this wood joint.\nUnlock the object first.\nBoth grips navigate.");
        override_hint(&mut status, &mut instruction, "Trigger selects the section handle.");
        assert!(!text(&status, &instruction, "").contains("wood joint"));
    }

}
