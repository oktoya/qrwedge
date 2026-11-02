# QR Wedge

웹캠/폰카로 QR·바코드를 읽어 바코드 스캐너처럼 키보드 입력 + Enter를 보낸다. 제작: OKTO

소스는 `src/main.rs` 하나로 Windows·macOS 공용 (Rust).

지원 코드: QR, Micro QR, DataMatrix, PDF417, Aztec, MaxiCode, Code128, Code39, Code93, EAN-13/8, UPC-A/E, ITF, Codabar, RSS (rxing). 반전(검은 바탕 흰 코드)도 읽는다.

## 쓰는 법
1. 실행하면 카메라 미리보기 창이 뜬다. 상태(카메라 번호, 마지막으로 읽은 코드)는 창 제목에 나온다.
2. 입력받을 프로그램(엑셀, 메모장 등)을 클릭해 둔다.
3. 코드를 카메라에 비추면 삑 소리와 함께 입력 + Enter.
- 미리보기 창 클릭: 다음 카메라. 종료: 창 닫기 또는 작업표시줄 아이콘 우클릭 > 창 닫기.
- 키 입력은 실제 USB 스캐너처럼 실제 키(키 코드 + 스캔 코드, 필요하면 Shift)로 누른다. 웹에서 `e.code`·`e.keyCode`가 실제 키보드와 같다. 한글처럼 키로 칠 수 없는 글자만 유니코드로 보낸다.
- 한글 입력 상태여도 영문이 섞인 코드는 입력하는 동안만 영문 상태로 바꿨다가 되돌린다. Caps Lock이 켜져 있어도 대소문자가 그대로 들어간다.
- 같은 코드를 계속 비추면 다시 입력하지 않는다. 화면에서 1.5초 이상 뺐다가 다시 비추면 또 입력한다.

## 옵션
`--cam N` 시작 카메라 · `--suffix enter|tab|none` · `--cooldown 1.5` · `--delay ms` 글자 사이 지연 · `--mirror` 좌우 반전

`QRWedge /?` (또는 `/help`, `--help`)로 실행하면 이 옵션 설명이, `/v` (또는 `/version`, `--version`)면 버전이 나온다 (Windows: 메시지 창, 맥: 터미널 출력 — `dist/QRWedge.app/Contents/MacOS/QRWedge --help`).

## 빌드
- Windows: `build_win.bat` → `dist\QRWedge.exe`
  - 필요: Rust `stable-x86_64-pc-windows-gnullvm` 툴체인(`C:\rust`), llvm-mingw(`C:\rust\mingw`)
- macOS (맥에서): `bash build_mac.sh` → `dist/QRWedge.app`
  - 필요: Xcode 명령줄 도구, Rust
  - 처음 실행 때 시스템 설정 > 개인정보 보호 및 보안에서 카메라·손쉬운 사용 허용

## 아이콘
`assets/make_icon.py`로 `assets/icon.png`(맥)·`assets/icon.ico`(Windows)를 만든다 (Pillow 필요). Windows 아이콘·제작자 정보는 `assets/qrwedge.rc`.

## 개발용 테스트
`cargo run --release --example selftest -- decode <PGM 폴더>`
