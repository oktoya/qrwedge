//! 개발용 자체 테스트 (배포 exe에는 들어가지 않는다)
//!   cargo run --release --example selftest -- decode <PGM 폴더>
//!   cargo run --release --example selftest -- type "<문자열>"   (1.5초 뒤 포커스된 창에 입력)
#![allow(dead_code, unused_attributes)]
#[path = "../src/main.rs"]
mod app;

use std::time::{Duration, Instant};

fn read_pgm(path: &std::path::Path) -> app::Frame {
    let data = std::fs::read(path).unwrap();
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while data[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !data[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&data[s..i]).to_string());
    }
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    let px = &data[i + 1..i + 1 + w * h];
    app::Frame { w, h, rgb: px.iter().flat_map(|&g| [g, g, g]).collect() }
}

fn bring_to_front(title: &str) -> bool {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let wide: Vec<u16> = title.encode_utf16().chain([0]).collect();
    unsafe {
        let target = FindWindowW(std::ptr::null(), wide.as_ptr());
        if target.is_null() {
            return false;
        }
        let fg_thread = GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
        let me = GetCurrentThreadId();
        AttachThreadInput(me, fg_thread, 1);
        SetForegroundWindow(target);
        AttachThreadInput(me, fg_thread, 0);
        std::thread::sleep(Duration::from_millis(300));
        GetForegroundWindow() == target
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args[1].as_str() {
        "decode" => {
            let mut hints = app::new_hints();
            let mut paths: Vec<_> = std::fs::read_dir(&args[2]).unwrap().map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "pgm")).collect();
            paths.sort();
            for p in paths {
                let f = read_pgm(&p);
                let want = std::fs::read_to_string(p.with_extension("txt")).unwrap();
                let t = Instant::now();
                let got = app::decode(&f, &mut hints).map(|r| r.text).unwrap_or_default();
                let ms = t.elapsed().as_millis();
                let ok = if got == want { "OK" } else { "FAIL" };
                println!("{:<12} {ok} {ms}ms  got={got:?}", p.file_stem().unwrap().to_string_lossy());
            }
        }
        "type" => {
            // args[3] 제목의 창을 앞으로 가져온 뒤, 그 창이 맨 앞일 때만 입력한다 (다른 창에 잘못 치지 않게)
            std::thread::sleep(Duration::from_millis(500));
            if !bring_to_front(&args[3]) {
                eprintln!("target window not in front; not typing");
                std::process::exit(2);
            }
            let mut kb = app::keys::Keyboard::new().unwrap();
            app::keys::type_text(&mut kb, &args[2], app::Suffix::Enter, 0);
        }
        _ => {}
    }
}
