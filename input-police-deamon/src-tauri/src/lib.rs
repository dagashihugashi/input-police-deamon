use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use std::time::Duration;
use std::thread;
use std::net::UdpSocket;

// Windows API のインポート
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SendMessageW, WM_IME_CONTROL};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_NONCONVERT, KEYBD_EVENT_FLAGS,
};

const IMC_GETCONVERSIONMODE: usize = 1;
const IMC_SETCONVERSIONMODE: usize = 2;

// --- 1. Windows APIによる「入力警察」のコア機能 ---

unsafe fn get_ime_wnd() -> HWND {
    let hwnd = GetForegroundWindow();
    ImmGetDefaultIMEWnd(hwnd)
}

fn enforce_half_width() {
    unsafe {
        let hime = get_ime_wnd();
        if hime.0 == 0 { return; }

        // 現在のIMEステータスを取得
        let status = SendMessageW(
            hime,
            WM_IME_CONTROL,
            WPARAM(IMC_GETCONVERSIONMODE as usize),
            LPARAM(0),
        );

        let current_mode = status.0 as u32;

        // ビット演算で現在の状態を判定
        let is_native = (current_mode & 1) != 0;      // 日本語モード(ひらがな等)か
        let is_fullshape = (current_mode & 8) != 0;   // 全角か

        // 「日本語入力ではなく、かつ全角になっている（＝全角アルファベット）」場合のみ全角フラグを折る
        if !is_native && is_fullshape {
            let new_mode = current_mode & !8;
            SendMessageW(
                hime,
                WM_IME_CONTROL,
                WPARAM(IMC_SETCONVERSIONMODE as usize),
                LPARAM(new_mode as isize),
            );
            println!("🚨 全角アルファベットを検知！半角に強制矯正しました。");
        }
    }
}

fn force_ime_off() {
    unsafe {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_NONCONVERT, // 無変換キー
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0), // キーを押す
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_NONCONVERT,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP, // キーを離す
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        
        // OSに対して直接「無変換キー」の入力をシミュレートする
        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        println!("🚓 Vimからの通報！無変換キーをシミュレートしてIMEを強制OFFにしました。");
    }
}

// --- 2. 警察の任務（バックグラウンド監視＆無線待機） ---

fn start_police_tasks() {
    // 任務A: パトロール（全角アルファベット撲滅）
    thread::spawn(|| {
        loop {
            enforce_half_width();
            // 30ミリ秒ごとにパトロール（人間の手より早く、CPU負荷はほぼゼロ）
            thread::sleep(Duration::from_millis(30));
        }
    });

    // 任務B: 無線待機（Vimからの通報を受け取る）
    thread::spawn(|| {
        // ポート51234番でUDP（無線）待機
        if let Ok(socket) = UdpSocket::bind("127.0.0.1:51234") {
            let mut buf = [0; 10];
            loop {
                if let Ok((size, _)) = socket.recv_from(&mut buf) {
                    let msg = String::from_utf8_lossy(&buf[..size]);
                    if msg.trim() == "ESC" {
                        force_ime_off();
                    }
                }
            }
        }
    });
}

// --- 3. Tauri本体の起動処理 ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // アプリ起動と同時にバックグラウンド任務を開始！
    start_police_tasks();

    tauri::Builder::default()
        .setup(|app| {
            let quit_i = MenuItem::with_id(app, "quit", "警察の任務を終了", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_i])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|_app, event| match event.id.as_ref() {
                    "quit" => {
                        println!("任務を終了します...");
                        std::process::exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}