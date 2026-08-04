use std::env;
use std::net::UdpSocket;

fn main() {
    // 実行時の引数を取得する
    let args: Vec<String> = env::args().collect();

    // 引数がない場合（obtainIMCmd として呼ばれた場合）
    if args.len() == 1 {
        // VSCodeに「今は日本語(1041)だよ」と嘘をつき、必ず切り替え処理を誘発させる
        print!("1041");
        return;
    }

    // 引数がある場合（switchIMCmd として呼ばれた場合）
    // デーモンに向けて「ESC」シグナルをUDPで送信する
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        let _ = socket.send_to(b"ESC", "127.0.0.1:51234");
    }
    
    // 切り替え完了の合図として英数(1033)を返す
    print!("1033");
}