// A press on the Mods button asks for the Mods screen, which the Options screen shows. Anything
// else is the title screen's own.
fn mod_menu_title_click(
    title: &mut Title,
    x: f32,
    y: f32,
    size: (f32, f32),
) -> ::core::ops::ControlFlow<(Action, bool)> {
    use core::ops::ControlFlow;
    use crate::title::native_mod_menu_title as m;
    if !matches!(title.mode, Mode::Main) {
        return ControlFlow::Continue(());
    }
    let (w, h) = (size.0.ceil(), size.1.ceil());
    let hit = title
        .buttons(w, h)
        .into_iter()
        .find(|b| b.enabled && x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + 20.0)
        .map(|b| b.id);
    if hit == Some(m::BUTTON_ID) {
        m::open();
        // The game opens Options; its draw and input hooks show the Mods screen instead.
        return ControlFlow::Break((Action::Options, true));
    }
    ControlFlow::Continue(())
}
