// Ctrl+A, C, V and X in the search box.
fn mod_menu_options_shortcut(
    game: &mut crate::game::Game<'_>,
    shortcut: horizon_client::clipboard::EditShortcut,
) -> ::core::ops::ControlFlow<bool> {
    if !horizon_client::title::native_mod_menu_title::is_open() {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(crate::game::native_mod_menu_ui::shortcut(game, shortcut))
}
