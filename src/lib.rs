//! 共有ライブラリコア。すべてのプラットフォームで共通するバックエンド・DB管理を格納する。

pub mod errors;
pub mod infra;
pub mod message;

use infra::db::DbManager;

/// JNI/FFI用エントリーポイント。
/// モバイルOS起動時、KotlinやSwiftが取得した「安全なアプリ専用ディレクトリのパス」をここに渡してDB接続プールを初期化する。
/// 呼出側で戻り値のポインタのライフサイクル管理が必要。
#[unsafe(no_mangle)]
pub extern "C" fn initialize_backend_from_path(db_path_raw: *const std::os::raw::c_char) -> *mut DbManager {
    if db_path_raw.is_null() {
        return std::ptr::null_mut();
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(db_path_raw) };
    match c_str.to_str() {
        Ok(path_str) => {
            match DbManager::new(path_str) {
                Ok(manager) => Box::into_raw(Box::new(manager)),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// JNI/FFI用クリーンアップエントリーポイント。
/// initialize_backend_from_path で生成したマネージャオブジェクトを安全にドロップする。
#[unsafe(no_mangle)]
pub extern "C" fn free_backend_manager(manager: *mut DbManager) {
    if !manager.is_null() {
        unsafe {
            let _ = Box::from_raw(manager);
        }
    }
}
