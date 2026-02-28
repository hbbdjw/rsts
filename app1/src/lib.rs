#![cfg_attr(feature = "simd", feature(portable_simd))]
pub mod modules;

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rsts_CsvExporter_export(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) -> jstring {
    let s = match env.get_string(&json) {
        Ok(v) => v,
        Err(_) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "invalid json");
            return std::ptr::null_mut();
        }
    };
    let json_str = s.to_string_lossy().into_owned();
    let rt = match tokio::runtime::Runtime::new() {
        Ok(v) => v,
        Err(_) => {
            let _ = env.throw_new("java/lang/RuntimeException", "runtime init failed");
            return std::ptr::null_mut();
        }
    };
    match rt.block_on(crate::modules::demo::pg_to_csv::pg_export_from_json(
        &json_str,
    )) {
        Ok(_) => match env.new_string("OK") {
            Ok(js) => js.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(e) => {
            let _ = env.throw_new(
                "java/lang/RuntimeException",
                format!("CSV export failed: {}", e),
            );
            std::ptr::null_mut()
        }
    }
}
