// A Mods button after Accessibility. The icon row moves left half a button to stay centred.
// `buttons` draws, hovers and presses all of them, so the new one is a button like the rest.
fn mod_menu_title_buttons(title: &Title, w: f32, h: f32) -> ::core::ops::ControlFlow<Vec<Button>> {
    use core::ops::ControlFlow;
    use crate::title::native_mod_menu_title as m;
    // The call below goes through this hook again: that one runs the original.
    if m::BUTTONS.enter() {
        return ControlFlow::Continue(());
    }
    let mut buttons = title.buttons(w, h);
    m::BUTTONS.leave();
    if matches!(title.mode, Mode::Main) {
        for b in buttons.iter_mut() {
            if matches!(b.id, 5..=7) {
                b.x -= m::ROW_SHIFT;
            }
        }
        if let Some((x, y)) = buttons.iter().find(|b| b.id == 7).map(|b| (b.x, b.y)) {
            // No label: the icon is drawn over it (`mod_menu_title_draw`).
            buttons.push(Button {
                x: x + 24.0,
                y,
                w: 20.0,
                label: String::new(),
                enabled: true,
                id: m::BUTTON_ID,
                icon: None,
                trailing: None,
            });
        }
    }
    ControlFlow::Break(buttons)
}
