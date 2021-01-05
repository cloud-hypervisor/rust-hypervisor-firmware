#[cfg(feature = "rom")]
global_asm!(include_str!("reset.s"));

global_asm!(include_str!("rom16.s"));
global_asm!(include_str!("rom32.s"));
global_asm!(include_str!("ram32.s"));
