fn enough_items_panel(game: &mut crate::game::Game<'_>, mod_id: &str, event: &mod_api::PanelEvent) {
    crate::game::enough_items::panel_event(game, mod_id, event);
}
