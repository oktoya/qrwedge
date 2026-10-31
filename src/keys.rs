//! 키 입력: 진짜 USB 스캐너처럼 실제 키(키 코드 + 스캔 코드, 필요하면 Shift)를 누른다.
//! 그래야 웹페이지에서 e.code / e.keyCode가 실제 키보드와 똑같이 나온다.
//! 키보드로 칠 수 없는 글자(한글 등)만 유니코드 문자로 보낸다.
//! 한글 입력 상태면 영문이 ㅁㅠㅊ처럼 들어가므로, 입력하는 동안만 영문 상태로 바꿨다가 되돌린다.

use std::time::Duration;

use super::Suffix;

pub use imp::Keyboard;

pub fn type_text(kb: &mut Keyboard, text: &str, suffix: Suffix, delay: u64) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let ime = if text.chars().any(|c| c.is_ascii_alphabetic()) { kb.force_english() } else { None };
    for ch in text.chars() {
        match ch {
            '\n' => kb.enter(),
            '\t' => kb.tab(),
            c => {
                if !kb.ascii_key(c) {
                    kb.unicode(c);
                }
            }
        }
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
    }
    match suffix {
        Suffix::Enter => kb.enter(),
        Suffix::Tab => kb.tab(),
        Suffix::None => {}
    }
    if let Some(state) = ime {
        // 보낸 키가 대상 프로그램에서 다 처리된 뒤에 되돌려야 한다
        std::thread::sleep(Duration::from_millis(150));
        kb.restore(state);
    }
}

#[cfg(windows)]
mod imp {
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const WM_IME_CONTROL: u32 = 0x0283;
    const IMC_GETCONVERSIONMODE: usize = 1;
    const IMC_SETCONVERSIONMODE: usize = 2;
    const IME_CMODE_NATIVE: usize = 1;

    pub struct Keyboard;

    pub struct ImeState {
        wnd: HWND,
        mode: usize,
    }

    fn input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 },
            },
        }
    }

    fn send(events: &[INPUT]) {
        unsafe { SendInput(events.len() as u32, events.as_ptr(), size_of::<INPUT>() as i32) };
    }

    /// 입력받는 창의 키보드 배열
    fn layout() -> HKL {
        unsafe { GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), null_mut())) }
    }

    fn scan_code(vk: VIRTUAL_KEY, hkl: HKL) -> u16 {
        unsafe { MapVirtualKeyExW(vk as u32, MAPVK_VK_TO_VSC, hkl) as u16 }
    }

    fn click(vk: VIRTUAL_KEY, hkl: HKL) -> [INPUT; 2] {
        let scan = scan_code(vk, hkl);
        [input(vk, scan, 0), input(vk, scan, KEYEVENTF_KEYUP)]
    }

    /// 입력받는 창(포커스된 컨트롤)의 IME 창
    fn ime_window() -> Option<HWND> {
        unsafe {
            let fg = GetForegroundWindow();
            if fg.is_null() {
                return None;
            }
            let mut gti: GUITHREADINFO = zeroed();
            gti.cbSize = size_of::<GUITHREADINFO>() as u32;
            let tid = GetWindowThreadProcessId(fg, null_mut());
            let focus = if GetGUIThreadInfo(tid, &mut gti) != 0 && !gti.hwndFocus.is_null() { gti.hwndFocus } else { fg };
            let wnd = ImmGetDefaultIMEWnd(focus);
            (!wnd.is_null()).then_some(wnd)
        }
    }

    fn ime_control(wnd: HWND, cmd: usize, value: usize) -> Option<usize> {
        let mut result = 0usize;
        let ok = unsafe {
            SendMessageTimeoutW(wnd, WM_IME_CONTROL, cmd, value as isize, SMTO_ABORTIFHUNG, 300, &mut result)
        };
        (ok != 0).then_some(result)
    }

    impl Keyboard {
        pub fn new() -> Option<Self> {
            Some(Keyboard)
        }

        pub fn enter(&mut self) {
            send(&click(VK_RETURN, layout()));
        }

        pub fn tab(&mut self) {
            send(&click(VK_TAB, layout()));
        }

        /// 현재 키보드 배열로 칠 수 있는 글자면 실제 키로 누르고 true
        pub fn ascii_key(&mut self, c: char) -> bool {
            if !c.is_ascii() || c.is_ascii_control() {
                return false;
            }
            let hkl = layout();
            let r = unsafe { VkKeyScanExW(c as u16, hkl) };
            if r == -1 {
                return false;
            }
            let (vk, mods) = ((r & 0xFF) as VIRTUAL_KEY, (r >> 8) & 0xFF);
            if mods & !1 != 0 {
                return false; // Ctrl/Alt가 필요한 글자는 유니코드로
            }
            let mut shift = mods & 1 != 0;
            // Caps Lock이 켜져 있으면 영문 대소문자가 뒤집히므로 Shift를 반대로
            if c.is_ascii_alphabetic() && unsafe { GetKeyState(VK_CAPITAL as i32) } & 1 != 0 {
                shift = !shift;
            }
            let shift_scan = scan_code(VK_LSHIFT, hkl);
            let mut events = Vec::with_capacity(4);
            if shift {
                events.push(input(VK_LSHIFT, shift_scan, 0));
            }
            events.extend(click(vk, hkl));
            if shift {
                events.push(input(VK_LSHIFT, shift_scan, KEYEVENTF_KEYUP));
            }
            send(&events);
            true
        }

        pub fn unicode(&mut self, c: char) {
            let mut buf = [0u16; 2];
            let events: Vec<INPUT> = c
                .encode_utf16(&mut buf)
                .iter()
                .flat_map(|&u| [input(0, u, KEYEVENTF_UNICODE), input(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)])
                .collect();
            send(&events);
        }

        /// 한글 입력 상태면 영문으로 바꾸고 원래 상태를 돌려준다
        pub fn force_english(&mut self) -> Option<ImeState> {
            let wnd = ime_window()?;
            let mode = ime_control(wnd, IMC_GETCONVERSIONMODE, 0)?;
            if mode & IME_CMODE_NATIVE == 0 {
                return None;
            }
            ime_control(wnd, IMC_SETCONVERSIONMODE, mode & !IME_CMODE_NATIVE)?;
            Some(ImeState { wnd, mode })
        }

        pub fn restore(&mut self, s: ImeState) {
            ime_control(s.wnd, IMC_SETCONVERSIONMODE, s.mode);
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::time::Duration;

    use enigo::{Direction, Enigo, Key, Keyboard as _, Settings};

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn TISCopyCurrentKeyboardInputSource() -> *mut c_void;
        fn TISCopyCurrentASCIICapableKeyboardInputSource() -> *mut c_void;
        fn TISSelectInputSource(source: *mut c_void) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFEqual(a: *const c_void, b: *const c_void) -> u8;
        fn CFRelease(p: *const c_void);
    }

    const KC_RETURN: u16 = 36;
    const KC_TAB: u16 = 48;

    pub struct Keyboard(Enigo);

    pub struct ImeState(*mut c_void);

    /// 미국 배열 기준 맥 키 코드 (USB 스캐너도 미국 배열 위치로 보낸다)
    fn us_key(c: char) -> Option<(u16, bool)> {
        const SHIFTED: &str = "~!@#$%^&*()_+{}|:\"<>?";
        const BASE: &str = "`1234567890-=[]\\;',./";
        let (base, shift) = if c.is_ascii_uppercase() {
            (c.to_ascii_lowercase(), true)
        } else if let Some(i) = SHIFTED.find(c) {
            (BASE.as_bytes()[i] as char, true)
        } else {
            (c, false)
        };
        let code = match base {
            'a' => 0, 's' => 1, 'd' => 2, 'f' => 3, 'h' => 4, 'g' => 5, 'z' => 6, 'x' => 7,
            'c' => 8, 'v' => 9, 'b' => 11, 'q' => 12, 'w' => 13, 'e' => 14, 'r' => 15,
            'y' => 16, 't' => 17, '1' => 18, '2' => 19, '3' => 20, '4' => 21, '6' => 22,
            '5' => 23, '=' => 24, '9' => 25, '7' => 26, '-' => 27, '8' => 28, '0' => 29,
            ']' => 30, 'o' => 31, 'u' => 32, '[' => 33, 'i' => 34, 'p' => 35, 'l' => 37,
            'j' => 38, '\'' => 39, 'k' => 40, ';' => 41, '\\' => 42, ',' => 43, '/' => 44,
            'n' => 45, 'm' => 46, '.' => 47, ' ' => 49, '`' => 50,
            _ => return None,
        };
        Some((code, shift))
    }

    impl Keyboard {
        pub fn new() -> Option<Self> {
            Enigo::new(&Settings::default()).ok().map(Keyboard)
        }

        pub fn enter(&mut self) {
            let _ = self.0.raw(KC_RETURN, Direction::Click);
        }

        pub fn tab(&mut self) {
            let _ = self.0.raw(KC_TAB, Direction::Click);
        }

        pub fn ascii_key(&mut self, c: char) -> bool {
            let Some((code, shift)) = us_key(c) else { return false };
            if shift {
                let _ = self.0.key(Key::Shift, Direction::Press);
            }
            let _ = self.0.raw(code, Direction::Click);
            if shift {
                let _ = self.0.key(Key::Shift, Direction::Release);
            }
            true
        }

        pub fn unicode(&mut self, c: char) {
            let _ = self.0.text(&c.to_string());
        }

        /// 한글 등 영문이 아닌 입력 소스면 영문(ABC 등)으로 바꾸고 원래 소스를 돌려준다
        pub fn force_english(&mut self) -> Option<ImeState> {
            unsafe {
                let cur = TISCopyCurrentKeyboardInputSource();
                let ascii = TISCopyCurrentASCIICapableKeyboardInputSource();
                if cur.is_null() || ascii.is_null() || CFEqual(cur, ascii) != 0 {
                    for p in [cur, ascii] {
                        if !p.is_null() {
                            CFRelease(p);
                        }
                    }
                    return None;
                }
                TISSelectInputSource(ascii);
                CFRelease(ascii);
                std::thread::sleep(Duration::from_millis(50));
                Some(ImeState(cur))
            }
        }

        pub fn restore(&mut self, s: ImeState) {
            unsafe {
                TISSelectInputSource(s.0);
                CFRelease(s.0);
            }
        }
    }
}
