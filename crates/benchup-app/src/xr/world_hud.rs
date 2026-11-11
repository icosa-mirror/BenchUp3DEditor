//! Compose world instructions from the actual XR mode, without duplicate actions.
pub(super) fn text<'a>(status: &'a str, instruction: &'a str, navigation: &'a str) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    for block in [status, instruction, navigation] {
        for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if !parts.contains(&line) { parts.push(line); }
        }
    }
    parts.join("\n")
}

pub(super) fn override_hint(status: &mut String, instruction: &mut String, hint: &str) {
    *status = status.lines().filter(|line| !instruction.lines().any(|old| old.trim() == line.trim()))
        .collect::<Vec<_>>().join("\n");
    *instruction = hint.trim().to_owned();
    if !instruction.is_empty() {
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
        override_hint(&mut status, &mut instruction, "Wood joint #9: no overlap\nTrigger edits this wood joint.");
        assert_eq!(text(&status, &instruction, "Both grips navigate."),
            "Wood joint #9: no overlap\nTrigger edits this wood joint.\nUnlock the object first.\nBoth grips navigate.");
        override_hint(&mut status, &mut instruction, "Trigger selects the section handle.");
        assert!(!text(&status, &instruction, "").contains("#9"));
    }

}
