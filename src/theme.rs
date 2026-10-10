use eframe::egui::Color32;

pub const CANVAS: Color32 = Color32::from_rgb(2, 4, 10);
pub const HUD: Color32 = Color32::from_rgb(6, 8, 14);
pub const TEXT: Color32 = Color32::from_rgb(226, 232, 240);
pub const MUTED: Color32 = Color32::from_rgb(100, 116, 139);
pub const EDGE: Color32 = Color32::from_rgb(30, 41, 59);
pub const FOCUS: Color32 = Color32::from_rgb(191, 219, 254);

pub const PRIMARY: Color32 = Color32::from_rgb(59, 130, 246);
pub const SECONDARY: Color32 = Color32::from_rgb(139, 92, 246);

pub const PROMPT: Color32 = Color32::from_rgb(224, 242, 254);
pub const PLAYBOOK: Color32 = Color32::from_rgb(220, 38, 38);
pub const SKILL: Color32 = Color32::from_rgb(139, 92, 246);
pub const PRINCIPLE: Color32 = Color32::from_rgb(34, 197, 94);
pub const MODEL: Color32 = Color32::from_rgb(59, 130, 246);

pub const HIT: Color32 = Color32::from_rgb(34, 197, 94);
pub const MISS: Color32 = Color32::from_rgb(220, 38, 38);
pub const UNK: Color32 = Color32::from_rgb(217, 119, 6);

pub const TYPE_HUD: f32 = 13.0;
pub const TYPE_TITLE: f32 = 15.0;
pub const TYPE_LABEL: f32 = 12.0;

pub const HUD_W: f32 = 256.0;
pub const INSPECT_W: f32 = 268.0;
pub const PAD: f32 = 16.0;
pub const PILL_R: u8 = 8;
pub const TRAVEL: Color32 = Color32::from_rgb(224, 242, 254);
pub const TRAVEL_GLOW: Color32 = Color32::from_rgb(59, 130, 246);
pub const TRAIL: usize = 14;
pub const TRAIL_STEP: f64 = 0.045;
pub const HOP_SECS: f64 = 0.7;
pub const CHAT_LABEL: &str = "chat";
pub const FLASH_SECS: f32 = 0.45;
pub const NODE_MIN: f32 = 2.1;
pub const NODE_GAIN: f32 = 0.7;
pub const CHAT_R: f32 = 3.4;
pub const NODE_HALO: f32 = 1.35;
pub const RING: f32 = 150.0;
pub const EDGE_K: f32 = 0.82;
pub const GRAVITY: f32 = 0.16;
pub const CLEAR: f32 = 16.0;
pub const ITERS: u32 = 110;

pub fn wash(color: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}
