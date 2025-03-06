use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{TrayIconBuilder, menu::Menu, menu::MenuEvent, menu::MenuItem};
use wry::WebViewBuilder;
mod whatSelect;
use whatSelect::get_selected_files;

enum UserEvent {
    // TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
}
fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let builder = WebViewBuilder::new()
        .with_url("http://tauri.app")
        .with_drag_drop_handler(|e| {
            match e {
                wry::DragDropEvent::Enter { paths, position } => {
                    println!("DragEnter: {position:?} {paths:?} ")
                }
                wry::DragDropEvent::Over { position } => println!("DragOver: {position:?} "),
                wry::DragDropEvent::Drop { paths, position } => {
                    println!("DragDrop: {position:?} {paths:?} ")
                }
                wry::DragDropEvent::Leave => println!("DragLeave"),
                _ => {}
            }

            true
        });

    // Tray icon
    let quit_menu = MenuItem::new("Quit", true, None);
    let tray_menu = Menu::new();
    tray_menu.append(&quit_menu).unwrap();
    let mut tray_icon = None;
    // let tray_icon = TrayIconBuilder::new()
    //     .with_menu(Box::new(tray_menu))
    //     .with_tooltip("Quick Look")
    //     // .with_icon(icon)
    //     .build()
    //     .unwrap();
    //イベントを登録
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        if let Err(_e) = proxy.send_event(UserEvent::MenuEvent(event)) {
            eprintln!("Failed to send event");
        }
    }));

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let _webview = builder.build(&window)?;
    //set_memory_usage_level(wry::MemoryUsageLevel::Low)
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let _webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().unwrap();
        builder.build_gtk(vbox)?
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            //最初に走る処理
            Event::NewEvents(tao::event::StartCause::Init) => {
                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu.clone()))
                        .with_tooltip("Quick Look")
                        // .with_icon(icon)
                        .build()
                        .unwrap(),
                );
                //おまじない?
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc2_core_foundation::{CFRunLoopGetMain, CFRunLoopWakeUp};

                    let rl = CFRunLoopGetMain().unwrap();
                    CFRunLoopWakeUp(&rl);
                }
                // 別スレッドで10秒後にget_selected_filesを呼び出し結果をprintl!する
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    match get_selected_files() {
                        Ok(files) => {
                            for file in files {
                                println!("{}", file);
                            }
                        }
                        Err(e) => {
                            eprintln!("{:?}", e);
                        }
                    }
                });
            }
            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                println!("{event:?}");

                if event.id == quit_menu.id() {
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                }
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }

            _ => {}
        }
    });
}
