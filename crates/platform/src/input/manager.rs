use std::collections::HashMap;

use crate::input::convert::{convert_winit_key, convert_winit_mouse};
use crate::input::state::{GamepadState, KeyboardState, MouseState};
use crate::input::types::{GamepadId, InputEvent, TouchPhase, TouchPoint};

pub struct InputManager {
    keyboard: KeyboardState,
    mouse: MouseState,
    text_input: crate::input::types::TextInputState,
    touch: crate::input::types::TouchState,
    gamepads: HashMap<GamepadId, GamepadState>,
    pending_events: Vec<InputEvent>,
    recording: bool,
    recorded: Vec<InputEvent>,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            keyboard: KeyboardState::new(),
            mouse: MouseState::new(),
            text_input: crate::input::types::TextInputState::default(),
            touch: crate::input::types::TouchState::default(),
            gamepads: HashMap::new(),
            pending_events: Vec::new(),
            recording: false,
            recorded: Vec::new(),
        }
    }

    pub fn start_capture(&mut self) { self.recording = true; }
    pub fn stop_capture(&mut self) { self.recording = false; self.recorded.clear(); }
    pub fn drain_captured(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.recorded)
    }

    pub fn push_event(&mut self, event: InputEvent) {
        if self.recording {
            self.recorded.push(event.clone());
        }
        self.pending_events.push(event);
    }

    pub fn poll(&mut self) {
        let events = std::mem::take(&mut self.pending_events);
        for event in &events {
            match event {
                InputEvent::KeyPress(key) => self.keyboard.press(*key),
                InputEvent::KeyRelease(key) => self.keyboard.release(*key),
                InputEvent::MouseMove(x, y) => self.mouse.move_to(*x, *y),
                InputEvent::MouseButton(btn, pressed) => {
                    if *pressed { self.mouse.press(*btn) } else { self.mouse.release(*btn) }
                }
                InputEvent::MouseScroll(x, y) => self.mouse.scroll_to(*x, *y),
                InputEvent::RawMouseMotion(x, y) => self.mouse.add_raw_delta(*x, *y),
                InputEvent::GamepadButton(id, btn, pressed) => {
                    let state = self.gamepads.entry(*id).or_default();
                    state.buttons.insert(*btn, *pressed);
                }
                InputEvent::GamepadAxis(id, axis, value) => {
                    let state = self.gamepads.entry(*id).or_default();
                    state.axes.insert(*axis, *value);
                }
                InputEvent::Text(_) => {}
                InputEvent::ImeEnabled | InputEvent::ImeDisabled |
                InputEvent::ImePreedit(_, _) | InputEvent::ImeCommit(_) => {}
                InputEvent::Touch { id, phase, x, y, force } => {
                    match phase {
                        TouchPhase::Started => {
                            let pt = TouchPoint { id: *id, x: *x, y: *y, force: *force };
                            self.touch.active.insert(*id, pt);
                            self.touch.started.push(pt);
                        }
                        TouchPhase::Moved => {
                            if let Some(pt) = self.touch.active.get_mut(id) {
                                pt.x = *x;
                                pt.y = *y;
                                pt.force = *force;
                            }
                        }
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            if let Some(pt) = self.touch.active.remove(id) {
                                self.touch.ended.push(pt);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn end_tick(&mut self) {
        self.keyboard.end_tick();
        self.mouse.end_tick();
        self.text_input.committed.clear();
        self.touch.prev_active.clear();
        for (id, pt) in &self.touch.active {
            self.touch.prev_active.insert(*id, (pt.x, pt.y));
        }
        self.touch.started.clear();
        self.touch.ended.clear();
        self.touch.pinch_delta = 0.0;
        self.touch.two_finger_scroll = (0.0, 0.0);
    }

    pub fn keyboard(&self) -> &KeyboardState { &self.keyboard }
    pub fn mouse(&self) -> &MouseState { &self.mouse }
    pub fn text_input(&self) -> &crate::input::types::TextInputState { &self.text_input }
    pub fn touch(&self) -> &crate::input::types::TouchState { &self.touch }
    pub fn gamepad(&self, id: GamepadId) -> Option<&GamepadState> {
        self.gamepads.get(&id)
    }

    pub fn handle_winit_event(&mut self, event: &winit::event::WindowEvent) {
        match event {
            winit::event::WindowEvent::KeyboardInput { event: key_event, .. } => {
                if let Some(keycode) = convert_winit_key(&key_event.physical_key) {
                    let e = if key_event.state == winit::event::ElementState::Pressed {
                        InputEvent::KeyPress(keycode)
                    } else {
                        InputEvent::KeyRelease(keycode)
                    };
                    self.push_event(e);
                }
                // Only emit Text events when IME is not active; IME handles text via ImeCommit.
                if !self.text_input.enabled {
                    if key_event.state == winit::event::ElementState::Pressed {
                        if let Some(ref txt) = key_event.text {
                            for ch in txt.chars() {
                                self.push_event(InputEvent::Text(ch));
                            }
                        }
                    }
                }
            }
            winit::event::WindowEvent::Ime(ime) => {
                match ime {
                    winit::event::Ime::Enabled => {
                        self.text_input.enabled = true;
                        self.push_event(InputEvent::ImeEnabled);
                    }
                    winit::event::Ime::Disabled => {
                        self.text_input.enabled = false;
                        self.text_input.preedit.clear();
                        self.text_input.preedit_cursor = None;
                        self.push_event(InputEvent::ImeDisabled);
                    }
                    winit::event::Ime::Preedit(text, cursor) => {
                        self.text_input.preedit = text.clone();
                        self.text_input.preedit_cursor = *cursor;
                        self.push_event(InputEvent::ImePreedit(text.clone(), *cursor));
                    }
                    winit::event::Ime::Commit(text) => {
                        self.text_input.committed.push_str(text);
                        self.push_event(InputEvent::ImeCommit(text.clone()));
                    }
                }
            }
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                self.push_event(InputEvent::MouseMove(position.x as f32, position.y as f32));
            }
            winit::event::WindowEvent::MouseInput { state, button, .. } => {
                let btn = convert_winit_mouse(*button);
                self.push_event(InputEvent::MouseButton(btn, *state == winit::event::ElementState::Pressed));
            }
            winit::event::WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        self.push_event(InputEvent::MouseScroll(*x, *y));
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        self.push_event(InputEvent::MouseScroll(pos.x as f32, pos.y as f32));
                    }
                }
            }
            winit::event::WindowEvent::Touch(touch) => {
                let phase = match touch.phase {
                    winit::event::TouchPhase::Started => TouchPhase::Started,
                    winit::event::TouchPhase::Moved => TouchPhase::Moved,
                    winit::event::TouchPhase::Ended => TouchPhase::Ended,
                    winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
                };
                let force = touch.force.map(|f| f.normalized() as f32);
                self.push_event(InputEvent::Touch {
                    id: touch.id,
                    phase,
                    x: touch.location.x as f32,
                    y: touch.location.y as f32,
                    force,
                });
            }
            _ => {}
        }
    }

    /// After all touch events have been polled, compute two-finger gesture deltas.
    pub fn compute_touch_gestures(&mut self) {
        let touch = &mut self.touch;
        if touch.active.len() == 2 {
            let ids: Vec<u64> = touch.active.keys().copied().collect();
            let prev = &touch.prev_active;
            if prev.len() == 2 {
                let cur0 = touch.active[&ids[0]];
                let cur1 = touch.active[&ids[1]];
                let p0 = prev[&ids[0]];
                let p1 = prev[&ids[1]];
                let prev_dx = p1.0 - p0.0;
                let prev_dy = p1.1 - p0.1;
                let cur_dx = cur1.x - cur0.x;
                let cur_dy = cur1.y - cur0.y;
                let prev_dist = (prev_dx * prev_dx + prev_dy * prev_dy).sqrt().max(1.0);
                let cur_dist = (cur_dx * cur_dx + cur_dy * cur_dy).sqrt().max(1.0);
                touch.pinch_delta = cur_dist - prev_dist;
                // Two-finger scroll: average motion of both points
                let avg_x = ((cur0.x - p0.0) + (cur1.x - p1.0)) * 0.5;
                let avg_y = ((cur0.y - p0.1) + (cur1.y - p1.1)) * 0.5;
                touch.two_finger_scroll = (avg_x, avg_y);
            }
        }
    }
}
