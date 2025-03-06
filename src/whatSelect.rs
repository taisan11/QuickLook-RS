use windows::{
    Win32::System::Variant::VARIANT,
    core::*, Win32::System::Com::*, Win32::UI::Shell::*,
};

const CLSID_SHELL_APPLICATION: GUID = GUID::from_u128(0x13709620c27911cea49e444553540000);

pub fn get_selected_files() -> Result<Vec<String>> {
    unsafe {
        // COM を初期化し、HRESULTをResultに変換
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()?; // COM 初期化に失敗した場合はエラーを返す

        // `Shell.Application` COM オブジェクトを取得
        let shell: IDispatch = CoCreateInstance(&CLSID_SHELL_APPLICATION, None, CLSCTX_INPROC_SERVER)?;

        // エクスプローラーウィンドウの一覧を取得
        let shell_windows: IShellWindows = shell.cast::<IShellWindows>()?;
        let mut selected_files = Vec::new();

        let count = shell_windows.Count()?;
        for i in 0..count {
            let index = VARIANT::from(i as i32);
            if let Ok(item) = shell_windows.Item(&index) {
                if let Some(dispatch) = item.cast::<IDispatch>().ok() {
                    if let Ok(folder_view) = dispatch.cast::<IShellFolderViewDual>() {
                        if let Ok(selection) = folder_view.SelectedItems() {
                            let selection_count = selection.Count()?;
                            for j in 0..selection_count {
                                if let Ok(file) = selection.Item(&VARIANT::from(j as i32)) {
                                    if let Ok(path) = file.Path() {
                                        selected_files.push(path.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // COM のクリーンアップ
        CoUninitialize();
        Ok(selected_files)
    }
}