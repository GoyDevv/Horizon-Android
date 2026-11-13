// While the Mods screen is asked for, it is drawn instead of the Options screen.
fn mod_menu_draw_options(game: &mut crate::game::Game<'_>, batch: &mut HudBatch) -> ::core::ops::ControlFlow<()> {
    if !horizon_client::title::native_mod_menu_title::is_open() {
        return ::core::ops::ControlFlow::Continue(());
    }
    crate::game::native_mod_menu_ui::draw(game, batch);
    ::core::ops::ControlFlow::Break(())
}
