//! QR Wedge — 웹캠/폰카로 QR·바코드를 읽어 바코드 스캐너처럼 키보드로 입력하고 Enter를 친다.
//!
//! 사용법:  QRWedge [--cam 0] [--suffix enter|tab|none] [--cooldown 1.5] [--delay 0] [--mirror]
//! - 입력받을 프로그램(엑셀, 메모장 등)을 클릭해 둔 채로 카메라에 코드를 비추면 된다.
//! - 미리보기 창을 클릭하면 다음 카메라로 바뀐다. 창을 닫으면 종료. 상태는 창 제목에 나온다.
//! - 키 입력은 실제 USB 스캐너처럼 실제 키로 누른다 (src/keys.rs).
//! - Windows: 미리보기 창은 항상 위에 있고 포커스를 뺏지 않는다. 작업표시줄 우클릭 > 창 닫기로도 종료.
//! - macOS: 시스템 설정 > 개인정보 보호 및 보안 > 손쉬운 사용·카메라 권한이 필요하다.
//!   미리보기 창을 클릭하면 그 창으로 포커스가 가므로, 다시 입력할 프로그램을 클릭해 둔다.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

pub mod keys;

use minifb::{MouseButton, ScaleMode, Window, WindowOptions};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType,
    Resolution,
};
use nokhwa::Camera;
use rxing::{helpers::detect_in_luma_slice_with_hints, DecodeHints};
use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};
use zune_jpeg::JpegDecoder;

const TITLE: &str = "QR Wedge";
const GREEN: u32 = 0x00_FF_00;
#[cfg(target_os = "macos")]
const NO_KEY: &str = "키 입력 권한 없음 (손쉬운 사용에서 QR Wedge 허용)";
#[cfg(not(target_os = "macos"))]
const NO_KEY: &str = "키 입력 권한 없음";

#[derive(Clone, Copy, PartialEq)]
pub enum Suffix {
    Enter,
    Tab,
    None,
}

struct Args {
    cam: usize,
    suffix: Suffix,
    cooldown: f64,
    delay: u64,
    mirror: bool,
}

fn parse_args() -> Args {
    let mut a = Args { cam: 0, suffix: Suffix::Enter, cooldown: 1.5, delay: 0, mirror: false };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--cam" => a.cam = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--suffix" => {
                a.suffix = match it.next().as_deref() {
                    Some("tab") => Suffix::Tab,
                    Some("none") => Suffix::None,
                    _ => Suffix::Enter,
                }
            }
            "--cooldown" => a.cooldown = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.5),
            "--delay" => a.delay = it.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--mirror" => a.mirror = true,
            "/?" | "/help" | "-?" | "-h" | "--help" => {
                show_help();
                std::process::exit(0);
            }
            _ => {}
        }
    }
    a
}

const HELP: &str = "\
QR Wedge 1.0.0 — 제작: OKTO
웹캠/폰카로 QR·바코드를 읽어 키보드로 입력하고 Enter를 친다.

사용법:  QRWedge [옵션]

  --cam N          시작 카메라 번호 (기본 0)
  --suffix 종류    코드 뒤에 칠 키: enter | tab | none (기본 enter)
  --cooldown 초    같은 코드를 다시 입력하기까지 화면에서 빠져 있어야 하는 시간 (기본 1.5)
  --delay ms       글자 사이 지연, 느린 프로그램용 (기본 0)
  --mirror         미리보기 좌우 반전
  /?  /help        이 도움말

예)  QRWedge --suffix tab --cam 1

미리보기 창 클릭: 다음 카메라 · 종료: 창 닫기";

/// Windows exe는 콘솔이 없으므로 메시지 창으로, 맥은 터미널에 출력한다
fn show_help() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (text, title) = (wide(HELP), wide(TITLE));
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONINFORMATION) };
    }
    #[cfg(not(windows))]
    println!("{HELP}");
}

fn beep() {
    #[cfg(windows)]
    std::thread::spawn(|| unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::Beep(2000, 60);
    });
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("afplay").arg("/System/Library/Sounds/Tink.aiff").spawn();
}

// ---- 미리보기 창 (Windows만, 항상 위는 minifb 옵션) ----
// 포커스를 뺏지 않게(NOACTIVATE) 하면 작업표시줄에서 빠지므로 APPWINDOW로 버튼을 되살리고,
// exe에 넣은 아이콘(리소스 1번)을 창·작업표시줄 아이콘으로 단다. 작업표시줄 우클릭 > 창 닫기로 종료.
#[cfg(windows)]
fn setup_window(win: &Window) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    let hwnd = win.get_window_handle();
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | (WS_EX_NOACTIVATE | WS_EX_APPWINDOW) as isize);
        let module = GetModuleHandleW(std::ptr::null());
        for (which, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
            let size = GetSystemMetrics(metric);
            let icon = LoadImageW(module, 1 as *const u16, IMAGE_ICON, size, size, LR_DEFAULTCOLOR);
            if !icon.is_null() {
                SendMessageW(hwnd, WM_SETICON, which as usize, icon as isize);
            }
        }
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}

#[cfg(not(windows))]
fn setup_window(_: &Window) {}

// ---- 카메라 ----
#[derive(Clone)]
pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

fn list_cameras() -> Vec<CameraIndex> {
    nokhwa::query(ApiBackend::Auto)
        .map(|v| v.into_iter().map(|c| c.index().clone()).collect())
        .unwrap_or_default()
}

/// 실패하면 마지막 오류 문구를 돌려준다 (창 제목에 보여 원인을 알 수 있게)
fn open_camera(index: &CameraIndex) -> Result<Camera, String> {
    // MJPEG로 받아야 USB2 웹캠이 720p에서도 프레임이 제대로 나온다 (YUY2는 5~10fps).
    // 맥 내장 카메라처럼 MJPEG가 없는 카메라는 다음 요청으로 넘어간다
    let want = CameraFormat::new(Resolution::new(1280, 720), FrameFormat::MJPEG, 30);
    let tries = [
        RequestedFormatType::Closest(want),
        RequestedFormatType::HighestResolution(Resolution::new(1280, 720)),
        RequestedFormatType::AbsoluteHighestFrameRate,
        RequestedFormatType::None,
    ];
    let mut err = String::from("열기 실패");
    for t in tries {
        let req = RequestedFormat::new::<RgbFormat>(t);
        match Camera::new(index.clone(), req).and_then(|mut cam| cam.open_stream().map(|_| cam)) {
            Ok(cam) => return Ok(cam),
            Err(e) => err = e.to_string(),
        }
    }
    Err(err)
}

fn try_open(cams: &[CameraIndex], pos: usize, err: &mut String) -> Option<Camera> {
    match open_camera(cams.get(pos)?) {
        Ok(cam) => Some(cam),
        Err(e) => {
            *err = e;
            None
        }
    }
}

fn grab(cam: &mut Camera) -> Option<Frame> {
    let buf = cam.frame().ok()?;
    if buf.source_frame_format() == FrameFormat::MJPEG {
        let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
        let mut dec = JpegDecoder::new_with_options(buf.buffer(), opts);
        let rgb = dec.decode().ok()?;
        let info = dec.info()?;
        Some(Frame { w: info.width as usize, h: info.height as usize, rgb })
    } else {
        let res = buf.resolution();
        let rgb = buf.decode_image::<RgbFormat>().ok()?.into_raw();
        Some(Frame { w: res.width() as usize, h: res.height() as usize, rgb })
    }
}

// ---- 바코드 ----
pub struct Hit {
    pub text: String,
    pub format: String,
    pub pts: Vec<(f32, f32)>,
}

pub fn new_hints() -> DecodeHints {
    DecodeHints { TryHarder: Some(true), AlsoInverted: Some(true), ..DecodeHints::default() }
}

pub fn decode(f: &Frame, hints: &mut DecodeHints) -> Option<Hit> {
    let luma: Vec<u8> = f
        .rgb
        .chunks_exact(3)
        .map(|p| ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8)
        .collect();
    let r = detect_in_luma_slice_with_hints(&luma, f.w as u32, f.h as u32, None, hints).ok()?;
    if r.getText().is_empty() {
        return None;
    }
    Some(Hit {
        text: r.getText().to_string(),
        format: r.getBarcodeFormat().to_string(),
        pts: r.getPoints().iter().map(|p| (p.x, p.y)).collect(),
    })
}

/// 코드 없는 복잡한 화면은 한 장에 ~200ms 걸려서, 인식은 별도 스레드에서 돌리고 미리보기는 카메라 속도로 그린다
fn spawn_decoder() -> (SyncSender<Frame>, Receiver<Option<Hit>>) {
    let (job_tx, job_rx) = mpsc::sync_channel::<Frame>(1);
    let (res_tx, res_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut hints = new_hints();
        for f in job_rx {
            if res_tx.send(decode(&f, &mut hints)).is_err() {
                break;
            }
        }
    });
    (job_tx, res_rx)
}

// ---- 그리기 ----
fn draw_line(buf: &mut [u32], w: usize, h: usize, a: (f32, f32), b: (f32, f32)) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1.0) as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let (px, py) = (x as i32 + dx, y as i32 + dy);
                if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                    buf[py as usize * w + px as usize] = GREEN;
                }
            }
        }
    }
}

fn draw_border(buf: &mut [u32], w: usize, h: usize, t: usize) {
    for y in 0..h {
        for x in 0..w {
            if x < t || y < t || x >= w - t || y >= h - t {
                buf[y * w + x] = GREEN;
            }
        }
    }
}

fn shorten(s: &str, n: usize) -> String {
    let s: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if s.chars().count() <= n { s } else { s.chars().take(n - 3).collect::<String>() + "..." }
}

fn main() {
    let args = parse_args();

    // 카메라 여는 데 몇 초 걸릴 수 있으니 창부터 띄운다
    let opts = WindowOptions {
        resize: true,
        topmost: true,
        scale_mode: ScaleMode::AspectRatioStretch,
        ..WindowOptions::default()
    };
    let Ok(mut window) = Window::new(TITLE, 640, 360, opts) else { return };
    setup_window(&window);
    let blank = vec![0x20_20_20u32; 640 * 360];
    window.set_title(&format!("{TITLE} — 카메라 여는 중..."));
    let _ = window.update_with_buffer(&blank, 640, 360);

    // 맥: 카메라 권한 요청은 비동기라, 답이 올 때까지 기다린 뒤에 카메라를 찾는다
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = mpsc::channel();
        nokhwa::nokhwa_initialize(move |ok| {
            let _ = tx.send(ok);
        });
        window.set_title(&format!("{TITLE} — 카메라 권한 확인 중..."));
        let deadline = Instant::now() + Duration::from_secs(120);
        while window.is_open() && Instant::now() < deadline && rx.try_recv().is_err() {
            let _ = window.update_with_buffer(&blank, 640, 360);
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    let mut kb = keys::Keyboard::new();
    let (job_tx, res_rx) = spawn_decoder();
    let mut busy = false;
    let mut overlay: Option<(Vec<(f32, f32)>, Instant)> = None;

    let mut cams = list_cameras();
    let mut pos = if cams.is_empty() { 0 } else { args.cam % cams.len() };
    let mut cam_err = String::new();
    let mut cam = try_open(&cams, pos, &mut cam_err);

    let mut last_text = String::new();
    let mut last_seen = Instant::now();
    let mut flash_until = Instant::now();
    let mut status = if kb.is_some() { "준비".to_string() } else { NO_KEY.to_string() };
    let mut title = String::new();
    let mut was_down = false;
    let mut disp: Vec<u32> = Vec::new();

    while window.is_open() {
        // 창 클릭: 다음 카메라 (목록을 새로 읽어 새로 꽂은 카메라도 잡는다)
        let down = window.get_mouse_down(MouseButton::Left);
        if down && !was_down {
            drop(cam.take());
            cams = list_cameras();
            pos = if cams.is_empty() { 0 } else { (pos + 1) % cams.len() };
            cam = try_open(&cams, pos, &mut cam_err);
        }
        was_down = down;

        let frame = cam.as_mut().and_then(grab);
        let cam_label = if cam.is_some() {
            format!("카메라 {pos}")
        } else if cams.is_empty() {
            "카메라 없음".to_string()
        } else {
            format!("카메라 {pos} 열기 실패: {}", shorten(&cam_err, 60))
        };
        let new_title = format!("{TITLE} — {cam_label} (창 클릭: 다음 카메라) | {status}");
        if new_title != title {
            window.set_title(&new_title);
            title = new_title;
        }

        let Some(f) = frame else {
            let _ = window.update_with_buffer(&blank, 640, 360);
            std::thread::sleep(Duration::from_millis(30));
            continue;
        };

        let now = Instant::now();
        if !busy && job_tx.try_send(f.clone()).is_ok() {
            busy = true;
        }
        while let Ok(hit) = res_rx.try_recv() {
            busy = false;
            let Some(hit) = hit else { continue };
            if hit.text != last_text || now.duration_since(last_seen).as_secs_f64() > args.cooldown {
                // 맥: 실행 중에 손쉬운 사용을 허용해도 다시 켜지 않고 바로 쓰도록 매번 다시 확인
                if kb.is_none() {
                    kb = keys::Keyboard::new();
                }
                if let Some(k) = kb.as_mut() {
                    keys::type_text(k, &hit.text, args.suffix, args.delay);
                }
                beep();
                flash_until = now + Duration::from_millis(300);
                status = format!("[{}] {}", hit.format, shorten(&hit.text, 40));
                if kb.is_none() {
                    status = format!("{status} | {NO_KEY}");
                }
                last_text = hit.text;
            }
            last_seen = now;
            overlay = Some((hit.pts, now + Duration::from_millis(250)));
        }

        let (w, h) = (f.w, f.h);
        disp.clear();
        disp.extend(f.rgb.chunks_exact(3).map(|p| (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32));
        if args.mirror {
            disp.chunks_exact_mut(w).for_each(|row| row.reverse());
        }
        if let Some((pts, until)) = &overlay
            && now < *until
        {
            let fx = |x: f32| if args.mirror { (w - 1) as f32 - x } else { x };
            let pts: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (fx(x), y)).collect();
            // 1D 바코드는 점 2개(선), QR 등은 꼭짓점들을 잇는 다각형
            let n = pts.len();
            let edges = if n == 2 { 1 } else { n };
            for i in 0..edges {
                draw_line(&mut disp, w, h, pts[i], pts[(i + 1) % n]);
            }
        }
        if now < flash_until {
            draw_border(&mut disp, w, h, 12);
        }
        let _ = window.update_with_buffer(&disp, w, h);
    }
}
