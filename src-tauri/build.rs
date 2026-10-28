// Declaring the app manifest makes every command opt-in per capability; the remote practice
// window gets none.
const COMMANDS: &[&str] = &[
    "get_courses", "get_profile", "save_profile", "unlock_courses", "has_access_code", "remove_access_code", "reset_app", "get_course", "get_lesson", "save_progress", "get_check", "get_final", "submit_quiz",
    "get_exercise", "submit_exercise", "get_messages", "ask_tutor",
    "get_settings", "save_settings", "set_api_key", "list_models", "test_provider", "get_certificate", "print_certificate", "open_practice", "place_practice", "close_practice", "practice_nav", "practice_tab", "get_stats", "check_update", "download_update", "apply_update", "get_whats_new", "mark_whats_new_seen", "get_app_tour", "mark_app_tour_seen", "app_version", "open_help", "get_sidekick_nudges", "get_sidekick_help", "get_coach", "set_coach_stage", "coach_turn", "screen_agent_run", "screen_agent_stop", "screen_agent_answer",
    "get_ai_usage", "set_ai_daily_cap", "get_sidekick_activity", "export_sidekick_activity",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build");
}
