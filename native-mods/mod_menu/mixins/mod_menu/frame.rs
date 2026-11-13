// If the Options screen is gone without the Mods screen having been left (a world loading, a
// disconnect), the next Options screen is the normal one.
fn mod_menu_frame(game: &mut crate::game::Game<'_>) {
    if horizon_client::title::native_mod_menu_title::is_open() && game.screen != crate::game::Screen::Options {
        horizon_client::title::native_mod_menu_title::close();
    }
}
