use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

// Windows API のインポート
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SendMessageW, WM_IME_CONTROL};

// 初期状態はオン（true）にしておく
static IS_ACTIVE: AtomicBool = AtomicBool::new(true);

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
        if hime.0 == 0 {
            return;
        }

        // 現在のIMEステータスを取得
        let status = SendMessageW(
            hime,
            WM_IME_CONTROL,
            WPARAM(IMC_GETCONVERSIONMODE as usize),
            LPARAM(0),
        );

        let current_mode = status.0 as u32;

        // ビット演算で現在の状態を判定
        let is_native = (current_mode & 1) != 0; // 日本語モード(ひらがな等)か
        let is_fullshape = (current_mode & 8) != 0; // 全角か

        // 「日本語入力ではなく、かつ全角になっている（＝全角アルファベット）」場合のみ全角フラグを折る
        if !is_native && is_fullshape {
            let new_mode = current_mode & !8;
            SendMessageW(
                hime,
                WM_IME_CONTROL,
                WPARAM(IMC_SETCONVERSIONMODE as usize),
                LPARAM(new_mode as isize),
            );
        }
    }
}

fn force_ime_off() {
    unsafe {
        let hime = get_ime_wnd();
        if hime.0 != 0 {
            // 定数 6 (IMC_SETOPENSTATUS) に対して 0 (OFF) を送信し、確実に半角英数(A)にする
            SendMessageW(hime, WM_IME_CONTROL, WPARAM(6 as usize), LPARAM(0 as isize));
        }
    }
}

// --- 2. 警察の任務（バックグラウンド監視＆無線待機） ---

fn start_police_tasks() {
    // 任務A: パトロール（全角アルファベット撲滅）
    thread::spawn(|| {
        loop {
            if IS_ACTIVE.load(Ordering::Relaxed) {
                enforce_half_width();
            }
            thread::sleep(Duration::from_millis(30));
        }
    });

    // 任務B: 無線待機（Vimからの通報を受け取る）
    thread::spawn(|| {
        let socket = UdpSocket::bind("127.0.0.1:51235").expect("UDPポートの確保に失敗しました！");
        let mut buf = [0; 10];
        loop {
            if let Ok((size, _)) = socket.recv_from(&mut buf) {
                if IS_ACTIVE.load(Ordering::Relaxed) {
                    let msg = String::from_utf8_lossy(&buf[..size]);
                    if msg.trim() == "ESC" {
                        force_ime_off();
                    }
                }
            }
        }
    });
}

// --- 3. Tauri本体の起動処理 (v2対応版) ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // アプリ起動と同時にバックグラウンド任務を開始！
    start_police_tasks();

    tauri::Builder::default()
        .setup(|app| {
            // メニューアイテムを作成
            let toggle_i = MenuItem::with_id(app, "toggle", "Pause", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            
            // メニューにセット
            let menu = Menu::with_items(app, &[&toggle_i, &quit_i])?;

            // メニューの文字を書き換えるためにクローンを持っておく
            let toggle_i_clone = toggle_i.clone();

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |_app, event| match event.id.as_ref() {
                    "toggle" => {
                        // 状態を反転
                        let current = IS_ACTIVE.load(Ordering::Relaxed);
                        IS_ACTIVE.store(!current, Ordering::Relaxed);
                        
                        // v2の書き方でメニューのテキストを動的に変更
                        if current {
                            let _ = toggle_i_clone.set_text("Restart");
                        } else {
                            let _ = toggle_i_clone.set_text("Pause");
                        }
                    }
                    "quit" => {
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