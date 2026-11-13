// Dragging does nothing on the Mods screen (the Options screen behind it must not feel it).
fn mod_menu_options_drag(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<Result<()>> {
    if !horizon_client::title::native_mod_menu_title::is_open() {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.dragging = false;
    ::core::ops::ControlFlow::Break(Ok(()))
}
