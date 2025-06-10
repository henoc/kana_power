#![windows_subsystem = "windows"]

use windows::{
    Win32::Foundation::*, 
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::LibraryLoader::GetModuleHandleA,
    Win32::UI::Input::KeyboardAndMouse::*,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tray_icon::{TrayIconBuilder, menu::{Menu, MenuEvent, MenuItem}, Icon};
use image::{DynamicImage, ImageBuffer, Rgba};
use std::sync::mpsc;
use log::{info, warn};

const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;

static LEFT_CTRL_PRESSED: AtomicBool = AtomicBool::new(false);
static RIGHT_CTRL_PRESSED: AtomicBool = AtomicBool::new(false);
static OTHER_KEY_PRESSED: AtomicBool = AtomicBool::new(false);
static SHOULD_SEND_IME_OFF: AtomicBool = AtomicBool::new(false);
static SHOULD_SEND_IME_ON: AtomicBool = AtomicBool::new(false);

fn get_key_name(vk_code: u32) -> &'static str {
    match vk_code {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x10 => "Shift",
        0x11 => "Ctrl",
        0x12 => "Alt",
        0x13 => "Pause",
        0x14 => "CapsLock",
        0x16 => "IME_ON",
        0x1A => "IME_OFF",
        0x1B => "Esc",
        0x20 => "Space",
        0x25 => "←",
        0x26 => "↑",
        0x27 => "→",
        0x28 => "↓",
        0x2E => "Delete",
        0x30..=0x39 => "0-9",
        0x41..=0x5A => "A-Z",
        0xA0 => "左Shift",
        0xA1 => "右Shift",
        0xA2 => "左Ctrl",
        0xA3 => "右Ctrl",
        0xA4 => "左Alt",
        0xA5 => "右Alt",
        _ => "その他",
    }
}

fn send_ime_off() {
    unsafe {
        let mut inputs: Vec<INPUT> = Vec::with_capacity(2);

        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = VK_IME_OFF;
        inputs.push(input);
        
        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = VK_IME_OFF;
        input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        inputs.push(input);

        let ret = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        info!("IME OFFキーを送信しました: {}", ret);
    }
}

fn send_ime_on() {
    unsafe {
        let mut inputs: Vec<INPUT> = Vec::with_capacity(2);

        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = VK_IME_ON;
        inputs.push(input);
        
        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki.wVk = VK_IME_ON;
        input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        inputs.push(input);

        let ret = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        info!("IME ONキーを送信しました: {}", ret);
    }
}

fn is_key_pressed(vk_code: VIRTUAL_KEY) -> bool {
    unsafe {
        GetAsyncKeyState(vk_code.0 as i32) < 0
    }
}

unsafe extern "system" fn hook_callback(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code >= 0 {
        let vk_code = l_param.0 as *const KBDLLHOOKSTRUCT;
        if !vk_code.is_null() {
            let key_code = (*vk_code).vkCode;
            match w_param.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    info!("キー押下: {} ({})", get_key_name(key_code), key_code);
                    if key_code == VK_LCONTROL.0 as u32 {
                        LEFT_CTRL_PRESSED.store(true, Ordering::SeqCst);
                    } else if key_code == VK_RCONTROL.0 as u32 {
                        RIGHT_CTRL_PRESSED.store(true, Ordering::SeqCst);
                    } else if LEFT_CTRL_PRESSED.load(Ordering::SeqCst) || RIGHT_CTRL_PRESSED.load(Ordering::SeqCst) {
                        OTHER_KEY_PRESSED.store(true, Ordering::SeqCst);
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => {
                    info!("キー解放: {} ({})", get_key_name(key_code), key_code);
                    if key_code == VK_LCONTROL.0 as u32 {
                        if !OTHER_KEY_PRESSED.load(Ordering::SeqCst) {
                            SHOULD_SEND_IME_OFF.store(true, Ordering::SeqCst);
                        }
                        LEFT_CTRL_PRESSED.store(false, Ordering::SeqCst);
                        OTHER_KEY_PRESSED.store(false, Ordering::SeqCst);
                    } else if key_code == VK_RCONTROL.0 as u32 {
                        if !OTHER_KEY_PRESSED.load(Ordering::SeqCst) {
                            SHOULD_SEND_IME_ON.store(true, Ordering::SeqCst);
                        }
                        RIGHT_CTRL_PRESSED.store(false, Ordering::SeqCst);
                        OTHER_KEY_PRESSED.store(false, Ordering::SeqCst);
                    }
                }
                _ => {}
            }
        }
    }
    CallNextHookEx(None, code, w_param, l_param)
}

fn ime_control_thread() {
    loop {
        if SHOULD_SEND_IME_OFF.load(Ordering::SeqCst) {
            if !is_key_pressed(VK_LCONTROL) {
                send_ime_off();
                SHOULD_SEND_IME_OFF.store(false, Ordering::SeqCst);
            }
        }
        if SHOULD_SEND_IME_ON.load(Ordering::SeqCst) {
            if !is_key_pressed(VK_RCONTROL) {
                send_ime_on();
                SHOULD_SEND_IME_ON.store(false, Ordering::SeqCst);
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn create_icon() -> Icon {
    // 16x16の白い「あ」の画像を作成
    let mut img = ImageBuffer::new(16, 16);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([255, 255, 255, 255]);
    }
    let rgba = DynamicImage::ImageRgba8(img).into_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    Icon::from_rgba(rgba.into_raw(), width, height).unwrap()
}

fn main() -> windows::core::Result<()> {
    // ログ設定
    simple_logging::log_to_file("kana_power.log", log::LevelFilter::Info).unwrap();
    
    info!("キー入力の監視を開始します。トレイアイコンを右クリックして終了できます。");
    info!("左Ctrlのみを押して離すとVK_IME_OFFを送信します。");
    
    // トレイアイコンの設定
    let (tx, rx) = mpsc::channel();
    let tray_menu = Menu::new();
    let quit_item = MenuItem::new("終了", true, None);
    tray_menu.append(&quit_item).unwrap();
    
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("かなパワー")
        .with_icon(create_icon())
        .build()
        .unwrap();

    // メニューイベントの監視
    let event_tx = tx.clone();
    MenuEvent::set_event_handler(Some(Box::new(move |_event| {
        event_tx.send(()).unwrap();
    })));

    // IME制御用スレッドを起動
    thread::spawn(|| {
        ime_control_thread();
    });

    unsafe {
        let instance = HINSTANCE(GetModuleHandleA(None)?.0);
        let hook = SetWindowsHookExA(
            WH_KEYBOARD_LL,
            Some(hook_callback),
            Some(instance),
            0,
        );

        if hook.is_err() {
            warn!("フックの設定に失敗しました");
            return Ok(());
        }

        let mut msg = MSG::default();
        while GetMessageA(&mut msg, None, 0, 0).into() {
            if rx.try_recv().is_ok() {
                // 終了メニューが選択された
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }

        let _ = UnhookWindowsHookEx(hook.unwrap());
        Ok(())
    }
}
