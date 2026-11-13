// The wheel on the Mods screen: the list or the detail pane, whichever the pointer is over.
fn mod_menu_options_scroll(game: &mut crate::game::Game<'_>, amount: f32) -> ::core::ops::ControlFlow<Result<()>> {
    if !horizon_client::title::native_mod_menu_title::is_open() {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(crate::game::native_mod_menu_ui::scroll(game, amount))
}
