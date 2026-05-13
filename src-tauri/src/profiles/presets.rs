//! Built-in per-app profiles seeded on first launch. Each points at a built-in
//! style by ID so the FK always resolves.

use chrono::Utc;

use crate::styles::presets as style_presets;

use super::AppProfile;

pub fn builtin_profiles() -> Vec<AppProfile> {
    let now = Utc::now().to_rfc3339();
    vec![
        builtin(
            "profile-slack",
            "com.tinyspeck.slackmacgap",
            "Slack",
            style_presets::BUILTIN_CASUAL_ID,
            &now,
        ),
        builtin(
            "profile-discord",
            "com.hnc.Discord",
            "Discord",
            style_presets::BUILTIN_CASUAL_ID,
            &now,
        ),
        builtin(
            "profile-mail",
            "com.apple.mail",
            "Mail",
            style_presets::BUILTIN_FORMAL_ID,
            &now,
        ),
        builtin(
            "profile-terminal",
            "com.apple.Terminal",
            "Terminal",
            style_presets::BUILTIN_PLAIN_LOWER_ID,
            &now,
        ),
        builtin(
            "profile-iterm2",
            "com.googlecode.iterm2",
            "iTerm2",
            style_presets::BUILTIN_PLAIN_LOWER_ID,
            &now,
        ),
        builtin(
            "profile-vscode",
            "com.microsoft.VSCode",
            "Visual Studio Code",
            style_presets::BUILTIN_CODE_IDENTIFIER_ID,
            &now,
        ),
        builtin(
            "profile-cursor",
            "com.todesktop.230313mzl4w4u92",
            "Cursor",
            style_presets::BUILTIN_CODE_IDENTIFIER_ID,
            &now,
        ),
    ]
}

fn builtin(
    id: &str,
    bundle_id: &str,
    display_name: &str,
    style_id: &str,
    now: &str,
) -> AppProfile {
    AppProfile {
        id: id.into(),
        bundle_id: bundle_id.into(),
        display_name: display_name.into(),
        enabled: true,
        style_id: style_id.into(),
        vocabulary: vec![],
        vocabulary_learning_override: None,
        created_at: now.into(),
        updated_at: now.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_have_unique_bundle_ids() {
        let presets = builtin_profiles();
        let mut bundles: Vec<&str> = presets.iter().map(|p| p.bundle_id.as_str()).collect();
        bundles.sort();
        bundles.dedup();
        assert_eq!(
            bundles.len(),
            presets.len(),
            "duplicate bundle id in built-in profiles"
        );
    }

    #[test]
    fn presets_reference_known_styles() {
        let presets = builtin_profiles();
        for preset in &presets {
            assert!(
                style_presets::BUILTIN_IDS.contains(&preset.style_id.as_str()),
                "preset {} references unknown style {}",
                preset.bundle_id,
                preset.style_id
            );
        }
    }
}
