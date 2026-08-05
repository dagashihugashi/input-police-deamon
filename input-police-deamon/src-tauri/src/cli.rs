use std::env;
use std::net::UdpSocket;

fn main() {
    let args: Vec<String> = env::args().collect();
    let args_str = args.join(" ");

    // VSCodeから切り替えの引数（1033）を渡された場合
    if args_str.contains("1033") {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            let _ = socket.send_to(b"ESC", "127.0.0.1:51235");
        }
        println!("1033"); // 改行付きで出力してVSCodeに確実に読み取らせる
    } else {
        // 現在の状態を聞かれた場合（obtainIMCmd）は、日本語であると答える
        println!("1041"); // 同上
    }
}