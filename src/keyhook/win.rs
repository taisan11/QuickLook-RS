mod windows_key_hook;

use std::{
    collections::HashSet,
    ffi::c_void,
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};
use tao::event::ElementState;
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleA,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, SetWindowsHookExA, UnhookWindowsHookEx, HC_ACTION, KBDLLHOOKSTRUCT,
            VIRTUAL_KEY, VK_SPACE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
        },
        WindowsAndMessaging::{CallNextHookEx, GetMessageA, MSG},
    },
};

type HookCallback = Box<dyn Fn(KeyHookEvent) + Send + 'static>;

/// キーボードフックからのイベント
#[derive(Clone, Debug)]
pub struct KeyHookEvent {
    /// キーコード (Windows VK_* 定数)
    pub virtual_key: u32,
    /// キーの状態
    pub state: ElementState,
    /// イベントが処理済みとしてマークされているかどうか
    pub handled: bool,
}

/// キーボードフックの登録エラー
#[derive(Debug)]
pub enum RegisterError {
    /// フックの設定に失敗
    HookFailed,
}

/// Windows APIを使用したグローバルキーボードフック
pub struct KeyboardHook {
    running: Arc<AtomicBool>,
    registered_keys: Arc<Mutex<HashSet<u32>>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

static mut HOOK_CALLBACK: Option<HookCallback> = None;
static mut HHOOK: Option<isize> = None;
static mut REGISTERED_KEYS: Option<Arc<Mutex<HashSet<u32>>>> = None;

impl KeyboardHook {
    /// 新しいキーボードフックを作成
    pub fn new() -> Self {
        KeyboardHook {
            running: Arc::new(AtomicBool::new(false)),
            registered_keys: Arc::new(Mutex::new(HashSet::new())),
            thread_handle: None,
        }
    }

    /// 監視するキーを登録
    pub fn register_key(&self, vk_code: u32) {
        let mut keys = self.registered_keys.lock().unwrap();
        keys.insert(vk_code);
    }

    /// 監視するキーを解除
    pub fn unregister_key(&self, vk_code: u32) {
        let mut keys = self.registered_keys.lock().unwrap();
        keys.remove(&vk_code);
    }

    /// キーボードフックを開始
    pub fn start<F>(&mut self, callback: F) -> Result<(), RegisterError>
    where
        F: Fn(KeyHookEvent) + Send + 'static,
    {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let registered_keys = self.registered_keys.clone();

        unsafe {
            HOOK_CALLBACK = Some(Box::new(callback));
            REGISTERED_KEYS = Some(registered_keys.clone());
        }

        self.thread_handle = Some(thread::spawn(move || {
            unsafe {
                let module = GetModuleHandleA(None).expect("GetModuleHandleA failed");
                let hook = SetWindowsHookExA(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    HINSTANCE(module.0),
                    0,
                );

                if hook.0 == 0 {
                    running.store(false, Ordering::SeqCst);
                    return;
                }

                HHOOK = Some(hook.0);

                let mut msg: MSG = mem::zeroed();
                while running.load(Ordering::SeqCst) {
                    if GetMessageA(&mut msg, HWND(0), 0, 0).0 > 0 {
                        // メッセージループを維持
                    } else {
                        break;
                    }
                }

                if let Some(hook_handle) = HHOOK {
                    UnhookWindowsHookEx(hook_handle.into());
                }
            }
        }));

        Ok(())
    }

    /// キーボードフックを停止
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        unsafe {
            HOOK_CALLBACK = None;
            HHOOK = None;
            REGISTERED_KEYS = None;
        }
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        self.stop();
    }
}

/// キーボードフックのコールバック関数
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        if let (Some(callback), Some(registered_keys)) = (&HOOK_CALLBACK, &REGISTERED_KEYS) {
            let kb_struct = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk_code = kb_struct.vkCode;
            
            // 登録されたキーのみを処理
            let should_process = {
                let keys = registered_keys.lock().unwrap();
                keys.is_empty() || keys.contains(&vk_code)
            };
            
            if should_process {
                let state = match wparam.0 as u32 {
                    WM_KEYDOWN => ElementState::Pressed,
                    WM_KEYUP => ElementState::Released,
                    _ => return CallNextHookEx(None, code, wparam, lparam),
                };

                let mut event = KeyHookEvent {
                    virtual_key: vk_code,
                    state,
                    handled: false,
                };

                callback(event.clone());

                if event.handled {
                    // イベントが処理された場合は、次のフックに渡さない
                    return LRESULT(1);
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}
