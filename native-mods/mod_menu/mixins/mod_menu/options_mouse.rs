// The mouse on the Mods screen.
fn mod_menu_options_mouse(
    game: &mut crate::game::Game<'_>,
    pressed: bool,
    right: bool,
) -> ::core::ops::ControlFlow<Result<()>> {
    if !horizon_client::title::native_mod_menu_title::is_open() {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(crate::game::native_mod_menu_ui::mouse(game, pressed, right))
}
