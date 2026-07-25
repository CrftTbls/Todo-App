//! 共有ライブラリコア。すべてのプラットフォームで共通するバックエンド・DB管理を格納する。

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod errors;
pub mod infra;
pub mod message;
pub mod features;

#[cfg(feature = "ffi")]
#[allow(unsafe_code)]
pub mod ffi {
    use super::infra::db::DbManager;

    /// JNI/FFI用エントリーポイント。
    /// モバイルOS起動時、KotlinやSwiftが取得した「安全なアプリ専用ディレクトリのパス」をここに渡してDB接続プールを初期化する。
    /// 呼出側で戻り値のポインタのライフサイクル管理が必要。
    ///
    /// # Safety
    /// `db_path_raw` must be a valid null-terminated C string pointer.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn initialize_backend_from_path(db_path_raw: *const std::os::raw::c_char) -> *mut DbManager {
        if db_path_raw.is_null() {
            return std::ptr::null_mut();
        }
        // SAFETY: db_path_raw is checked for null above and caller ensures valid C string pointer.
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
    ///
    /// # Safety
    /// `manager` must be a valid pointer created by `initialize_backend_from_path` or null.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn free_backend_manager(manager: *mut DbManager) {
        if !manager.is_null() {
            // SAFETY: manager was allocated by Box::into_raw and is non-null.
            drop(unsafe { Box::from_raw(manager) });
        }
    }
}
