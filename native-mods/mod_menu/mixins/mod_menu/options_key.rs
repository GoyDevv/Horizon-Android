// Keys on the Mods screen: typing searches, the arrows pick, Escape leaves.
fn mod_menu_options_key(
    game: &mut crate::game::Game<'_>,
    key: KeyCode,
    text: Option<&str>,
) -> ::core::ops::ControlFlow<Result<()>> {
    if !horizon_client::title::native_mod_menu_title::is_open() {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(crate::game::native_mod_menu_ui::key(game, key, text))
}
