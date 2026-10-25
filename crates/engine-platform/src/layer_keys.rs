//! 0.5.8 (#198): a layer surface's keyboard, through `xkbcommon-dl`.
//!
//! Wayland sends the keyboard layout as an xkb keymap in a file descriptor and the key
//! codes raw; turning a code into a symbol and the text it types is xkbcommon's job.
//! `smithay-client-toolkit` does it too, but through the `xkbcommon` crate, which links
//! libxkbcommon when the crate is built; a wheel must not depend on that library to
//! import. `xkbcommon-dl` (already here through `winit`) loads it the first time a
//! layer surface has a keyboard, and without it a layer surface simply gets no text.

use std::ffi::{CStr, c_char};
use std::os::fd::OwnedFd;

use engine_core::Modifiers;
use xkbcommon_dl::{
    XKB_MOD_NAME_ALT, XKB_MOD_NAME_CTRL, XKB_MOD_NAME_LOGO, XKB_MOD_NAME_SHIFT, xkb_context,
    xkb_context_flags, xkb_keymap, xkb_keymap_compile_flags, xkb_keymap_format, xkb_state,
    xkb_state_component, xkbcommon_option,
};

/// Wayland key codes are evdev's; xkb's are those plus 8.
const EVDEV_OFFSET: u32 = 8;

/// A compiled keymap and the state (which modifiers are down) that goes with it.
pub(crate) struct Keymap {
    context: *mut xkb_context,
    keymap: *mut xkb_keymap,
    state: *mut xkb_state,
}

// The pointers are only used from the layer thread, which owns the `Keymap`.
unsafe impl Send for Keymap {}

impl Keymap {
    /// Builds a keymap from the file descriptor `wl_keyboard.keymap` delivers (`size`
    /// bytes of text, NUL-terminated); `None` where libxkbcommon is missing or the
    /// keymap does not compile.
    pub(crate) fn from_fd(fd: OwnedFd, size: usize) -> Option<Self> {
        let xkb = xkbcommon_option()?;
        // SAFETY: the compositor promises `size` readable bytes in the descriptor; the
        // map is private and dropped at the end of this function.
        let map = unsafe {
            memmap2::MmapOptions::new()
                .len(size)
                .map_copy_read_only(&fd)
                .ok()?
        };
        let text = CStr::from_bytes_until_nul(&map).ok()?;
        // SAFETY: the calls follow xkbcommon's contract; null results are checked.
        unsafe {
            let context = (xkb.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
            if context.is_null() {
                return None;
            }
            let keymap = (xkb.xkb_keymap_new_from_string)(
                context,
                text.as_ptr(),
                xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            );
            if keymap.is_null() {
                (xkb.xkb_context_unref)(context);
                return None;
            }
            let state = (xkb.xkb_state_new)(keymap);
            if state.is_null() {
                (xkb.xkb_keymap_unref)(keymap);
                (xkb.xkb_context_unref)(context);
                return None;
            }
            Some(Self {
                context,
                keymap,
                state,
            })
        }
    }

    /// `wl_keyboard.modifiers`: the depressed, latched and locked modifier masks and
    /// the layout group.
    pub(crate) fn update_mask(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        if let Some(xkb) = xkbcommon_option() {
            // SAFETY: `state` is live for as long as `self`.
            unsafe {
                (xkb.xkb_state_update_mask)(self.state, depressed, latched, locked, 0, 0, group);
            }
        }
    }

    /// The keysym `code` (a Wayland key code) means now, with the modifiers down.
    pub(crate) fn keysym(&self, code: u32) -> u32 {
        let Some(xkb) = xkbcommon_option() else {
            return 0;
        };
        // SAFETY: `state` is live.
        unsafe { (xkb.xkb_state_key_get_one_sym)(self.state, code + EVDEV_OFFSET) }
    }

    /// The text `code` types now (empty for a key that types nothing).
    pub(crate) fn utf8(&self, code: u32) -> String {
        let Some(xkb) = xkbcommon_option() else {
            return String::new();
        };
        let mut buffer = [0 as c_char; 16];
        // SAFETY: `state` is live; the buffer's length is passed.
        let length = unsafe {
            (xkb.xkb_state_key_get_utf8)(
                self.state,
                code + EVDEV_OFFSET,
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if length <= 0 {
            return String::new();
        }
        let bytes: Vec<u8> = buffer[..(length as usize).min(buffer.len() - 1)]
            .iter()
            .map(|&c| c as u8)
            .collect();
        String::from_utf8(bytes).unwrap_or_default()
    }

    /// Whether holding `code` repeats it (a letter does, a modifier does not).
    pub(crate) fn repeats(&self, code: u32) -> bool {
        xkbcommon_option().is_some_and(|xkb| {
            // SAFETY: `keymap` is live.
            unsafe { (xkb.xkb_keymap_key_repeats)(self.keymap, code + EVDEV_OFFSET) != 0 }
        })
    }

    /// The modifier keys down now, as the engine's `Modifiers`.
    pub(crate) fn modifiers(&self) -> Modifiers {
        let Some(xkb) = xkbcommon_option() else {
            return Modifiers::default();
        };
        let active = |name: &[u8]| {
            // SAFETY: `state` is live; the names are NUL-terminated byte strings.
            unsafe {
                (xkb.xkb_state_mod_name_is_active)(
                    self.state,
                    name.as_ptr().cast(),
                    xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
                ) > 0
            }
        };
        Modifiers {
            shift: active(XKB_MOD_NAME_SHIFT),
            ctrl: active(XKB_MOD_NAME_CTRL),
            alt: active(XKB_MOD_NAME_ALT),
            meta: active(XKB_MOD_NAME_LOGO),
        }
    }
}

impl Drop for Keymap {
    fn drop(&mut self) {
        if let Some(xkb) = xkbcommon_option() {
            // SAFETY: each pointer was made by xkbcommon and is released once.
            unsafe {
                (xkb.xkb_state_unref)(self.state);
                (xkb.xkb_keymap_unref)(self.keymap);
                (xkb.xkb_context_unref)(self.context);
            }
        }
    }
}

/// The name `InputEvent::Key` carries for a keysym (see `key_name` for `winit`'s keys):
/// a named key in lowercase snake case (`arrow_left`, `f5`), or the character it types.
pub(crate) fn key_name(keysym: u32, text: &str) -> Option<String> {
    use xkbcommon_dl::keysyms as k;
    let named = match keysym {
        k::Return | k::KP_Enter | k::ISO_Enter => "enter",
        k::Tab | k::ISO_Left_Tab => "tab",
        k::space => "space",
        k::Escape => "escape",
        k::BackSpace => "backspace",
        k::Delete | k::KP_Delete => "delete",
        k::Insert | k::KP_Insert => "insert",
        k::Left | k::KP_Left => "arrow_left",
        k::Right | k::KP_Right => "arrow_right",
        k::Up | k::KP_Up => "arrow_up",
        k::Down | k::KP_Down => "arrow_down",
        k::Home | k::KP_Home => "home",
        k::End | k::KP_End => "end",
        k::Page_Up | k::KP_Page_Up => "page_up",
        k::Page_Down | k::KP_Page_Down => "page_down",
        k::Shift_L | k::Shift_R => "shift",
        k::Control_L | k::Control_R => "control",
        k::Alt_L | k::Alt_R => "alt",
        k::Super_L | k::Super_R | k::Meta_L | k::Meta_R => "super",
        k::Caps_Lock => "caps_lock",
        k::Num_Lock => "num_lock",
        k::Scroll_Lock => "scroll_lock",
        k::Menu => "context_menu",
        k::Print => "print_screen",
        k::Pause => "pause",
        k::F1..=k::F24 => return Some(format!("f{}", keysym - k::F1 + 1)),
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.to_string());
    }
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(text.to_string()),
        _ => None,
    }
}

/// The engine's own small key vocabulary (`engine_core::Key`) for a keysym.
pub(crate) fn engine_key(keysym: u32) -> Option<engine_core::Key> {
    use engine_core::Key;
    use xkbcommon_dl::keysyms as k;
    Some(match keysym {
        k::Tab | k::ISO_Left_Tab => Key::Tab,
        k::Return | k::KP_Enter | k::ISO_Enter => Key::Enter,
        k::space => Key::Space,
        k::Escape => Key::Escape,
        k::BackSpace => Key::Backspace,
        k::Delete | k::KP_Delete => Key::Delete,
        k::Left | k::KP_Left => Key::ArrowLeft,
        k::Right | k::KP_Right => Key::ArrowRight,
        k::Up | k::KP_Up => Key::ArrowUp,
        k::Down | k::KP_Down => Key::ArrowDown,
        k::Home | k::KP_Home => Key::Home,
        k::End | k::KP_End => Key::End,
        k::Page_Up | k::KP_Page_Up => Key::PageUp,
        k::Page_Down | k::KP_Page_Down => Key::PageDown,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use xkbcommon_dl::keysyms as k;

    #[test]
    fn named_keys_have_the_winit_path_names() {
        assert_eq!(key_name(k::Left, "").as_deref(), Some("arrow_left"));
        assert_eq!(key_name(k::Return, "\r").as_deref(), Some("enter"));
        assert_eq!(
            key_name(k::BackSpace, "\u{8}").as_deref(),
            Some("backspace")
        );
        assert_eq!(key_name(k::F5, "").as_deref(), Some("f5"));
        assert_eq!(key_name(k::F12, "").as_deref(), Some("f12"));
        assert_eq!(key_name(k::Page_Down, "").as_deref(), Some("page_down"));
    }

    #[test]
    fn a_character_key_is_named_by_what_it_types_and_a_silent_key_has_no_name() {
        assert_eq!(key_name(k::a, "a").as_deref(), Some("a"));
        assert_eq!(key_name(k::A, "A").as_deref(), Some("A"));
        assert_eq!(key_name(0, ""), None);
        // A control character is not a name.
        assert_eq!(key_name(k::c, "\u{3}"), None);
    }

    #[test]
    fn the_engines_key_vocabulary_is_the_same_small_set() {
        use engine_core::Key;
        assert_eq!(engine_key(k::Left), Some(Key::ArrowLeft));
        assert_eq!(engine_key(k::Return), Some(Key::Enter));
        assert_eq!(engine_key(k::KP_Enter), Some(Key::Enter));
        assert_eq!(engine_key(k::a), None);
        assert_eq!(engine_key(k::F5), None);
    }
}
