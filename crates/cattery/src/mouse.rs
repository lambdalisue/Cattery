//! The mouse as winit reports it, turned into what the session takes or
//! what the window does with it.

use felis_protocol::messages::{InputMods, MouseButton};
use winit::event::{MouseButton as WinitButton, MouseScrollDelta};
use winit::keyboard::ModifiersState;

/// The button a program is told of, where it has one.
pub fn button(button: WinitButton) -> Option<MouseButton> {
    todo!()
}

/// The modifiers a program is told were held.
pub fn mods(mods: ModifiersState) -> InputMods {
    todo!()
}

/// Whether a click goes to the program rather than to the window: while the
/// program asks for the mouse, unless Shift is held to select instead.
pub fn to_program(reporting: bool, mods: ModifiersState) -> bool {
    todo!()
}

/// What a turn of the wheel does.
#[derive(Debug, PartialEq, Eq)]
pub enum Wheel {
    /// Tell the program of these wheel buttons.
    Report(Vec<MouseButton>),
    /// Send these keys, for a pager on the alternate screen that does not
    /// ask for the mouse.
    Keys(Vec<u8>),
    /// Scroll the view this many rows into history, or out of it when
    /// negative.
    Scroll(i32),
}

/// What turning the wheel by `delta` does, given whether the program asks
/// for the mouse and is on the alternate screen. A line of the wheel
/// scrolls `lines` rows; pixels from a trackpad are kept in `residue`
/// until they make a row of `cell_height`.
pub fn wheel(
    delta: MouseScrollDelta,
    reporting: bool,
    alt_screen: bool,
    mods: ModifiersState,
    lines: f64,
    cell_height: f64,
    residue: &mut f64,
) -> Wheel {
    todo!()
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalPosition;

    use super::*;

    fn lines(y: f32) -> MouseScrollDelta {
        MouseScrollDelta::LineDelta(0.0, y)
    }

    fn pixels(y: f64) -> MouseScrollDelta {
        MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, y))
    }

    const NONE: ModifiersState = ModifiersState::empty();

    #[test]
    fn the_three_buttons_and_the_side_ones_are_told() {
        assert_eq!(button(WinitButton::Left), Some(MouseButton::Left));
        assert_eq!(button(WinitButton::Middle), Some(MouseButton::Middle));
        assert_eq!(button(WinitButton::Right), Some(MouseButton::Right));
        assert_eq!(button(WinitButton::Back), Some(MouseButton::Button8));
        assert_eq!(button(WinitButton::Forward), Some(MouseButton::Button9));
        assert_eq!(button(WinitButton::Other(20)), None);
    }

    #[test]
    fn shift_alt_and_control_are_told_and_command_is_not() {
        let all = ModifiersState::SHIFT
            | ModifiersState::ALT
            | ModifiersState::CONTROL
            | ModifiersState::SUPER;
        assert_eq!(mods(all), InputMods::SHIFT | InputMods::ALT | InputMods::CTRL);
        assert_eq!(mods(NONE), InputMods::empty());
    }

    #[test]
    fn a_click_goes_to_a_program_asking_for_the_mouse_unless_shift_is_held() {
        assert!(to_program(true, NONE));
        assert!(!to_program(true, ModifiersState::SHIFT));
        assert!(!to_program(false, NONE));
    }

    #[test]
    fn a_program_asking_for_the_mouse_is_told_of_each_notch() {
        assert_eq!(
            wheel(lines(2.0), true, false, NONE, 3.0, 20.0, &mut 0.0),
            Wheel::Report(vec![MouseButton::WheelUp, MouseButton::WheelUp]),
        );
        assert_eq!(
            wheel(lines(-1.0), true, true, NONE, 3.0, 20.0, &mut 0.0),
            Wheel::Report(vec![MouseButton::WheelDown]),
        );
    }

    #[test]
    fn a_pager_on_the_alternate_screen_gets_arrows_or_pages() {
        assert_eq!(
            wheel(lines(2.0), false, true, NONE, 3.0, 20.0, &mut 0.0),
            Wheel::Keys(b"\x1b[A\x1b[A".to_vec()),
        );
        assert_eq!(
            wheel(lines(-1.0), false, true, ModifiersState::SHIFT, 3.0, 20.0, &mut 0.0),
            Wheel::Keys(b"\x1b[6~".to_vec()),
        );
    }

    #[test]
    fn the_primary_screen_scrolls_its_history() {
        assert_eq!(
            wheel(lines(1.0), false, false, NONE, 3.0, 20.0, &mut 0.0),
            Wheel::Scroll(3)
        );
        assert_eq!(
            wheel(lines(-2.0), false, false, NONE, 3.0, 20.0, &mut 0.0),
            Wheel::Scroll(-6)
        );
    }

    #[test]
    fn shift_scrolls_history_even_while_a_program_asks_for_the_mouse() {
        assert_eq!(
            wheel(lines(1.0), true, false, ModifiersState::SHIFT, 3.0, 20.0, &mut 0.0),
            Wheel::Scroll(3),
        );
    }

    #[test]
    fn trackpad_pixels_add_up_to_rows() {
        let mut residue = 0.0;
        assert_eq!(
            wheel(pixels(15.0), false, false, NONE, 3.0, 20.0, &mut residue),
            Wheel::Scroll(0)
        );
        assert_eq!(
            wheel(pixels(15.0), false, false, NONE, 3.0, 20.0, &mut residue),
            Wheel::Scroll(1)
        );
        assert!((residue - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_trackpad_swipe_is_one_notch_to_a_program() {
        assert_eq!(
            wheel(pixels(-37.0), true, false, NONE, 3.0, 20.0, &mut 0.0),
            Wheel::Report(vec![MouseButton::WheelDown]),
        );
    }
}
