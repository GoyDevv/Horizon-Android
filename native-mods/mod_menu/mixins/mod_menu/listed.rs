// Mod Menu's row in Options > Mods. It has no keys and no settings.
fn mod_menu_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::native_mod_menu as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("{} {}", m::NAME, m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}
