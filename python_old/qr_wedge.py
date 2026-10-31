"""QR Wedge — 웹캠/폰카로 QR·바코드를 읽어 바코드 스캐너처럼 키보드로 입력하고 Enter를 친다.

사용법:  QRWedge [--cam 0] [--suffix enter|tab|none] [--cooldown 1.5] [--delay 0] [--mirror]
- 입력받을 프로그램(엑셀, 메모장 등)을 클릭해 둔 채로 카메라에 코드를 비추면 된다.
- 미리보기 창을 클릭하면 다음 카메라로 바뀐다. 창을 닫으면 종료.
- Windows: 미리보기 창은 항상 위에 있고 포커스를 뺏지 않는다.
- macOS: 시스템 설정 > 개인정보 보호 및 보안 > 손쉬운 사용·카메라 권한이 필요하다.
  미리보기 창을 클릭하면 그 창으로 포커스가 가므로, 다시 입력할 프로그램을 클릭해 둔다.
"""
import argparse
import ctypes
import subprocess
import sys
import time

import cv2
import numpy as np
import zxingcpp

WIN = "QR Wedge"
IS_MAC = sys.platform == "darwin"

# ---- 키보드 입력: Windows ----
if not IS_MAC:
    import winsound
    from ctypes import wintypes

    user32 = ctypes.WinDLL("user32", use_last_error=True)

    INPUT_KEYBOARD = 1
    KEYEVENTF_KEYUP = 0x0002
    KEYEVENTF_UNICODE = 0x0004
    VK_TAB = 0x09
    VK_RETURN = 0x0D

    class KEYBDINPUT(ctypes.Structure):
        _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD), ("dwFlags", wintypes.DWORD),
                    ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class MOUSEINPUT(ctypes.Structure):  # INPUT 크기를 맞추려고 둔다 (union 최대 멤버)
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD),
                    ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_size_t)]

    class _INPUTUNION(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("mi", MOUSEINPUT)]

    class INPUT(ctypes.Structure):
        _anonymous_ = ("u",)
        _fields_ = [("type", wintypes.DWORD), ("u", _INPUTUNION)]

    def _key(vk=0, scan=0, flags=0):
        return INPUT(type=INPUT_KEYBOARD, ki=KEYBDINPUT(wVk=vk, wScan=scan, dwFlags=flags))

    def press_special(name):
        code = VK_RETURN if name == "enter" else VK_TAB
        return [_key(vk=code), _key(vk=code, flags=KEYEVENTF_KEYUP)]

    def press_char(ch):
        events = []
        data = ch.encode("utf-16-le")
        for j in range(0, len(data), 2):  # 이모지 등 서로게이트 쌍도 처리
            unit = int.from_bytes(data[j:j + 2], "little")
            events += [_key(scan=unit, flags=KEYEVENTF_UNICODE),
                       _key(scan=unit, flags=KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]
        return events

    def post(events):
        user32.SendInput(len(events), (INPUT * len(events))(*events), ctypes.sizeof(INPUT))

    def beep():
        winsound.Beep(2000, 60)

    def can_type():
        return True

# ---- 키보드 입력: macOS ----
else:
    _as = ctypes.cdll.LoadLibrary(
        "/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices")
    _cf = ctypes.cdll.LoadLibrary("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
    _as.CGEventCreateKeyboardEvent.restype = ctypes.c_void_p
    _as.CGEventCreateKeyboardEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint16, ctypes.c_bool]
    _as.CGEventKeyboardSetUnicodeString.argtypes = [ctypes.c_void_p, ctypes.c_ulong,
                                                    ctypes.POINTER(ctypes.c_uint16)]
    _as.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
    _as.AXIsProcessTrusted.restype = ctypes.c_bool
    _cf.CFRelease.argtypes = [ctypes.c_void_p]
    kCGHIDEventTap = 0
    KC_RETURN, KC_TAB = 36, 48

    def press_special(name):
        code = KC_RETURN if name == "enter" else KC_TAB
        return [(code, None)]

    def press_char(ch):
        data = ch.encode("utf-16-le")
        units = [int.from_bytes(data[j:j + 2], "little") for j in range(0, len(data), 2)]
        return [(0, units)]

    def post(events):
        for code, units in events:
            for down in (True, False):
                ev = _as.CGEventCreateKeyboardEvent(None, code, down)
                if units:
                    buf = (ctypes.c_uint16 * len(units))(*units)
                    _as.CGEventKeyboardSetUnicodeString(ev, len(units), buf)
                _as.CGEventPost(kCGHIDEventTap, ev)
                _cf.CFRelease(ev)

    def beep():
        subprocess.Popen(["afplay", "/System/Library/Sounds/Tink.aiff"])

    def can_type():
        return _as.AXIsProcessTrusted()


def type_text(text, suffix, delay):
    keys = []  # 키 하나씩의 이벤트 묶음
    for ch in text.replace("\r\n", "\n").replace("\r", "\n"):
        if ch == "\n":
            keys.append(press_special("enter"))
        elif ch == "\t":
            keys.append(press_special("tab"))
        else:
            keys.append(press_char(ch))
    if suffix != "none":
        keys.append(press_special(suffix))
    if delay <= 0:
        post([e for k in keys for e in k])
        return
    for k in keys:
        post(k)
        time.sleep(delay / 1000)


# ---- 미리보기 창: 항상 위 + 포커스 안 뺏기 (Windows만) ----
def make_window_passive():
    if IS_MAC:
        return True
    hwnd = user32.FindWindowW(None, WIN)
    if not hwnd:
        return False
    GWL_EXSTYLE, WS_EX_TOPMOST, WS_EX_NOACTIVATE = -20, 0x8, 0x08000000
    style = user32.GetWindowLongW(hwnd, GWL_EXSTYLE)
    user32.SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE | WS_EX_TOPMOST)
    HWND_TOPMOST, SWP_NOSIZE, SWP_NOMOVE, SWP_NOACTIVATE = -1, 0x1, 0x2, 0x10
    user32.SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE)
    return True


# ---- 카메라 ----
def open_camera(index):
    cap = cv2.VideoCapture(index, cv2.CAP_AVFOUNDATION if IS_MAC else cv2.CAP_DSHOW)
    if not cap.isOpened():
        cap.release()
        return None
    # MJPG로 받아야 USB2 웹캠이 720p에서도 프레임이 제대로 나온다 (기본 YUY2는 5~10fps)
    cap.set(cv2.CAP_PROP_FOURCC, cv2.VideoWriter_fourcc(*"MJPG"))
    cap.set(cv2.CAP_PROP_FRAME_WIDTH, 1280)
    cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 720)
    return cap


def next_camera(start, max_index=6):
    for k in range(max_index):
        i = (start + k) % max_index
        cap = open_camera(i)
        if cap is not None:
            return i, cap
    return start, None


def ascii_safe(s, n=40):
    s = s.encode("ascii", "replace").decode()
    return s if len(s) <= n else s[:n - 3] + "..."


def main():
    ap = argparse.ArgumentParser(description="Webcam QR/barcode keyboard wedge")
    ap.add_argument("--cam", type=int, default=0, help="카메라 번호 (기본 0)")
    ap.add_argument("--suffix", choices=["enter", "tab", "none"], default="enter", help="입력 뒤에 칠 키")
    ap.add_argument("--cooldown", type=float, default=1.5,
                    help="같은 코드를 다시 입력하려면 화면에서 이 시간(초) 동안 사라져야 한다")
    ap.add_argument("--delay", type=int, default=0, help="글자 사이 지연(ms). 입력이 씹히는 프로그램용")
    ap.add_argument("--mirror", action="store_true", help="미리보기 좌우 반전")
    args = ap.parse_args()

    clicked = {"v": False}

    def on_mouse(event, *_):
        if event == cv2.EVENT_LBUTTONDOWN:
            clicked["v"] = True

    # 카메라 여는 데 몇 초 걸릴 수 있으니 창부터 띄운다
    cv2.namedWindow(WIN, cv2.WINDOW_NORMAL)
    cv2.resizeWindow(WIN, 640, 360)
    cv2.setMouseCallback(WIN, on_mouse)
    splash = np.zeros((360, 640, 3), np.uint8)
    cv2.putText(splash, "Opening camera...", (20, 180), cv2.FONT_HERSHEY_SIMPLEX, 0.8, (255, 255, 255), 2)
    cv2.imshow(WIN, splash)
    passive = make_window_passive()
    cv2.waitKey(1)

    cam, cap = next_camera(args.cam)

    last_text, last_seen, flash_until = "", 0.0, 0.0
    status = "ready" if can_type() else "NO ACCESSIBILITY PERMISSION"

    while True:
        if clicked["v"]:
            clicked["v"] = False
            if cap is not None:
                cap.release()
            cam, cap = next_camera(cam + 1)

        ok, frame = (cap.read() if cap is not None else (False, None))
        if not ok:
            frame = np.zeros((360, 640, 3), np.uint8)
            cv2.putText(frame, "No camera - click to retry", (20, 180),
                        cv2.FONT_HERSHEY_SIMPLEX, 0.8, (0, 0, 255), 2)
        else:
            now = time.monotonic()
            try:
                results = zxingcpp.read_barcodes(frame, text_mode=zxingcpp.TextMode.Plain)
            except Exception:
                results = []
            results = [r for r in results if r.valid and r.text]
            for r in results:
                p = r.position
                pts = [(p.top_left.x, p.top_left.y), (p.top_right.x, p.top_right.y),
                       (p.bottom_right.x, p.bottom_right.y), (p.bottom_left.x, p.bottom_left.y)]
                for a, b in zip(pts, pts[1:] + pts[:1]):
                    cv2.line(frame, a, b, (0, 255, 0), 3)
            if results:
                text = results[0].text
                if text != last_text or now - last_seen > args.cooldown:
                    type_text(text, args.suffix, args.delay)
                    beep()
                    last_text = text
                    flash_until = now + 0.3
                    status = f"[{results[0].format.name}] {ascii_safe(text)}"
                last_seen = now
            if args.mirror:
                frame = cv2.flip(frame, 1)
            if now < flash_until:
                cv2.rectangle(frame, (0, 0), (frame.shape[1] - 1, frame.shape[0] - 1), (0, 255, 0), 12)

        label = f"cam {cam} (click: next) | {status}"
        cv2.putText(frame, label, (10, 30), cv2.FONT_HERSHEY_SIMPLEX, 0.7, (0, 0, 0), 4)
        cv2.putText(frame, label, (10, 30), cv2.FONT_HERSHEY_SIMPLEX, 0.7, (255, 255, 255), 2)
        cv2.imshow(WIN, frame)
        if not passive:
            passive = make_window_passive()
        cv2.waitKey(1)
        if cv2.getWindowProperty(WIN, cv2.WND_PROP_VISIBLE) < 1:
            break

    if cap is not None:
        cap.release()
    cv2.destroyAllWindows()


if __name__ == "__main__":
    main()
