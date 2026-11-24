//! SDL3 gamepad backend. Owned and polled exclusively by VIctory's main thread.
use super::{publish_sdl_pads, PadState, A, B, DOWN, LEFT, LEFT_SHOULDER, LEFT_STICK, MENU, RIGHT, RIGHT_SHOULDER, RIGHT_STICK, UP, VIEW, X, Y};
use sdl3::gamepad::{Axis, Button, Gamepad};

pub struct SdlDriver {
    _sdl: sdl3::Sdl,
    subsystem: sdl3::GamepadSubsystem,
    pump: sdl3::EventPump,
    pads: Vec<Gamepad>,
    rumble: [(u16, u16); 4],
    rumble_refresh: std::time::Instant,
}

impl SdlDriver {
    /// Must be called on VIctory's main thread.
    pub fn new() -> Result<Self, String> {
        let sdl = sdl3::init().map_err(|e| e.to_string())?;
        let subsystem = sdl.gamepad().map_err(|e| e.to_string())?;
        let pump = sdl.event_pump().map_err(|e| e.to_string())?;
        let mut driver = Self { _sdl: sdl, subsystem, pump, pads: Vec::new(), rumble: [(0, 0); 4], rumble_refresh: std::time::Instant::now() };
        driver.poll();
        Ok(driver)
    }

    /// Poll on the same thread that constructed the driver.
    pub fn poll(&mut self) {
        self.pump.pump_events();
        self.subsystem.update();
        self.pads.retain(|pad| pad.connected());
        if let Ok(ids) = self.subsystem.gamepads() {
            for id in ids {
                if self.pads.iter().any(|pad| pad.id().ok() == Some(id)) { continue; }
                if let Ok(pad) = self.subsystem.open(id) {
                    eprintln!("SDL3 controller connected: {}", pad.name().unwrap_or_else(|| "Unknown gamepad".into()));
                    self.pads.push(pad);
                }
            }
        }
        let mut readings = [None; 4];
        for (slot, pad) in self.pads.iter().take(4).enumerate() {
            readings[slot] = Some(read(pad));
        }
        let refresh = self.rumble_refresh.elapsed() >= std::time::Duration::from_millis(500); for (slot, command) in super::take_sdl_rumble().into_iter().enumerate() { if let Some(levels) = command { self.rumble[slot] = levels; } if let Some(pad) = self.pads.get_mut(slot) { let (low, high) = self.rumble[slot]; if (command.is_some() || refresh) && (low != 0 || high != 0 || command.is_some()) { if let Err(e) = pad.set_rumble(low, high, 1000) { if command.is_some() { eprintln!("SDL3 vibration failed on slot {slot}: {e}"); } } } } else { self.rumble[slot] = (0, 0); } } if refresh { self.rumble_refresh = std::time::Instant::now(); }
        publish_sdl_pads(readings);
    }
}

fn read(pad: &Gamepad) -> PadState {
    let mapping = [
        (Button::Start, MENU), (Button::Back, VIEW),
        (Button::South, A), (Button::East, B), (Button::West, X), (Button::North, Y),
        (Button::DPadUp, UP), (Button::DPadDown, DOWN),
        (Button::DPadLeft, LEFT), (Button::DPadRight, RIGHT),
        (Button::LeftShoulder, LEFT_SHOULDER), (Button::RightShoulder, RIGHT_SHOULDER),
        (Button::LeftStick, LEFT_STICK), (Button::RightStick, RIGHT_STICK),
    ];
    let stick = |axis| (f32::from(pad.axis(axis)) / 32767.0).clamp(-1.0, 1.0);
    let trigger = |axis| (f32::from(pad.axis(axis)) / 32767.0).clamp(0.0, 1.0);
    PadState {
        buttons: mapping.iter().filter(|(button, _)| pad.button(*button)).fold(0, |bits, (_, bit)| bits | bit),
        left_trigger: trigger(Axis::TriggerLeft),
        right_trigger: trigger(Axis::TriggerRight),
        left_x: stick(Axis::LeftX),
        left_y: -stick(Axis::LeftY),
        right_x: stick(Axis::RightX),
        right_y: -stick(Axis::RightY),
    }
}
