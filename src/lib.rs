mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(unused)]

    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub mod ngx;

impl std::fmt::Display for bindings::NVSDK_NGX_Result {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chars = unsafe { bindings::GetNGXResultAsString(*self as _) } as *const i32;
        let length = unsafe { libc::wcslen(chars) };
        let string = unsafe { widestring::WideCString::from_ptr(chars.cast(), length) }
            .map_err(|_| std::fmt::Error)?;
        let string = string.to_string().map_err(|_| std::fmt::Error)?;
        f.write_str(&string)
    }
}

#[cfg(test)]
mod tests {
    use crate::bindings;

    #[test]
    fn test_error_message() {
        let string =
            bindings::NVSDK_NGX_Result::NVSDK_NGX_Result_FAIL_FeatureNotSupported.to_string();
        assert_eq!(string, "NVSDK_NGX_Result_FAIL_FeatureNotSupported");
    }
}
