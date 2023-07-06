//! Vulkan NGX.

#![deny(missing_docs)]

use std::mem::ManuallyDrop;

use crate::bindings;
use crate::Result;
use ash::vk::{self, Handle};

/// Returns a mutable pointer for [`ash::vk::Handle`].
fn ash_handle_to_pointer_mut<H: Handle + Copy, T>(ash_handle: &H) -> *mut T {
    let address = ash_handle.as_raw();
    let pointer = std::ptr::null_mut::<u8>();
    let pointer = unsafe { pointer.add(address as _) };
    pointer.cast()
}

fn convert_slice_of_strings_to_cstrings(data: &[String]) -> Result<Vec<std::ffi::CString>> {
    let strings: Vec<_> = data
        .iter()
        .cloned()
        .filter_map(|s| std::ffi::CString::new(s).ok())
        .collect();

    if strings.len() != data.len() {
        return Err(format!("Couldn't convert the extensions to CStrings.").into());
    }

    Ok(strings)
}

/// Vulkan extensions required for the NVIDIA NGX operation.
#[derive(Debug, Clone)]
pub struct RequiredExtensions {
    /// Vulkan device extensions required for NVIDIA NGX.
    pub device: Vec<String>,
    /// Vulkan instance extensions required for NVIDIA NGX.
    pub instance: Vec<String>,
}
impl RequiredExtensions {
    /// Returns a list of device extensions as a list of
    /// [`std::ffi::CString`].
    pub fn get_device_extensions_c_strings(&self) -> Result<Vec<std::ffi::CString>> {
        convert_slice_of_strings_to_cstrings(&self.device)
    }

    /// Returns a list of instance extensions as a list of
    /// [`std::ffi::CString`].
    pub fn get_instance_extensions_c_strings(&self) -> Result<Vec<std::ffi::CString>> {
        convert_slice_of_strings_to_cstrings(&self.instance)
    }

    /// Returns a list of required vulkan extensions for NGX to work.
    pub fn get() -> Result<Self> {
        let mut instance_extensions: *mut *const std::ffi::c_char = std::ptr::null_mut();
        let mut device_extensions: *mut *const std::ffi::c_char = std::ptr::null_mut();
        let mut instance_count = 0u32;
        let mut device_count = 0u32;
        Result::from(unsafe {
            bindings::NVSDK_NGX_VULKAN_RequiredExtensions(
                &mut instance_count as *mut _,
                &mut instance_extensions as *mut _,
                &mut device_count as *mut _,
                &mut device_extensions as *mut _,
            )
        })?;

        let mut instance = Vec::new();
        for i in 0..instance_count {
            instance.push(unsafe {
                std::ffi::CStr::from_ptr(*instance_extensions.add(i as usize))
                    .to_str()
                    .map(|s| s.to_owned())
                    .unwrap()
            });
        }

        let mut device = Vec::new();
        for i in 0..device_count {
            device.push(unsafe {
                std::ffi::CStr::from_ptr(*device_extensions.add(i as usize))
                    .to_str()
                    .map(|s| s.to_owned())
                    .unwrap()
            });
        }

        // unsafe {
        //     libc::free(device_extensions as _);
        //     libc::free(instance_extensions as _);
        // }

        Ok(Self { device, instance })
    }
}

/// Implementors of this trait can convert to a pointer of custom type
/// `T` from their [`ash::vk::Handle::as_raw`].
trait HandleToPointer<T> {
    /// Converts the raw handle to any pointer.
    ///
    /// # Safety
    ///
    /// The pointer converted isn't checked, so use it on your own risk.
    unsafe fn to_pointer_mut(&self) -> *mut T;
}

impl<T, H> HandleToPointer<T> for H
where
    H: Handle + Copy,
{
    unsafe fn to_pointer_mut(&self) -> *mut T {
        ash_handle_to_pointer_mut(self)
    }
}

/// NVIDIA NGX system.
#[derive(Debug)]
pub struct System {
    device: vk::Device,
}

/// Current [`ash::Entry`] with which the NGX was associated.
static mut ASH_ENTRY: Option<ManuallyDrop<ash::Entry>> = None;

/// Current [`ash::Instance`] with which the NGX was associated.
static mut ASH_INSTANCE: Option<ManuallyDrop<ash::Instance>> = None;

unsafe extern "C" fn get_instance_proc_addr<T>(
    instance: *mut T,
    proc_name: *const i8,
) -> Option<unsafe extern "C" fn()> {
    ASH_ENTRY
        .as_ref()
        .map(|e| {
            let instance = instance as *mut u8;
            let address = instance.offset_from(std::ptr::null::<u8>());
            let raw_handle = address as u64;
            e.get_instance_proc_addr(vk::Instance::from_raw(raw_handle), proc_name)
                .map(|p| std::mem::transmute(p))
        })
        .flatten()
}

unsafe extern "C" fn get_device_proc_addr<T>(
    logical_device: *mut T,
    proc_name: *const i8,
) -> Option<unsafe extern "C" fn()> {
    ASH_INSTANCE
        .as_ref()
        .map(|i| {
            let logical_device = logical_device as *mut u8;
            let address = logical_device.offset_from(std::ptr::null::<u8>());
            let raw_handle = address as u64;
            (i.fp_v1_0().get_device_proc_addr)(vk::Device::from_raw(raw_handle), proc_name)
                .map(|p| std::mem::transmute(p))
        })
        .flatten()
}

impl System {
    /// Creates a new NVIDIA NGX system.
    pub fn new(
        project_id: Option<uuid::Uuid>,
        engine_version: &str,
        application_data_path: &std::path::Path,
        entry: &ash::Entry,
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        logical_device: vk::Device,
    ) -> Result<Self> {
        unsafe {
            ASH_ENTRY = Some(ManuallyDrop::new(entry.clone()));
            ASH_INSTANCE = Some(ManuallyDrop::new(instance.clone()));
        }
        let engine_type = bindings::NVSDK_NGX_EngineType::NVSDK_NGX_ENGINE_TYPE_CUSTOM;
        let project_id = std::ffi::CString::new(
            project_id
                .unwrap_or_else(|| uuid::Uuid::new_v4())
                .to_string(),
        )
        .unwrap();
        let engine_version = std::ffi::CString::new(engine_version).unwrap();
        let application_data_path =
            widestring::WideString::from_str(application_data_path.to_str().unwrap());
        Result::from(unsafe {
            bindings::NVSDK_NGX_VULKAN_Init_with_ProjectID(
                project_id.as_ptr(),
                engine_type,
                engine_version.as_ptr(),
                application_data_path.as_ptr().cast(),
                instance.handle().to_pointer_mut(),
                physical_device.to_pointer_mut(),
                logical_device.to_pointer_mut(),
                Some(get_instance_proc_addr),
                Some(get_device_proc_addr),
                std::ptr::null(),
                bindings::NVSDK_NGX_Version::NVSDK_NGX_Version_API,
            )
        })
        .map(|_| Self {
            device: logical_device,
        })
    }

    fn shutdown(&self) -> Result {
        unsafe { bindings::NVSDK_NGX_VULKAN_Shutdown1(self.device.to_pointer_mut()) }.into()
    }

    /// Creates a new [`Feature`] with the logical device used to create
    /// this [`System`].
    pub fn create_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_type: bindings::NVSDK_NGX_Feature,
        parameters: Option<FeatureParameters>,
    ) -> Result<Feature> {
        let parameters = match parameters {
            Some(p) => p,
            None => FeatureParameters::get_capability_parameters()?,
        };
        Feature::new(self.device, command_buffer, feature_type, parameters)
    }

    /// Creates a supersampling feature.
    pub fn create_supersampling_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        parameters: Option<FeatureParameters>,
    ) -> Result<SuperSamplingFeature> {
        self.create_feature(
            command_buffer,
            bindings::NVSDK_NGX_Feature::NVSDK_NGX_Feature_SuperSampling,
            parameters,
        )
        .map(|f| unsafe { SuperSamplingFeature::from_feature_unchecked(f) })
    }
}

impl Drop for System {
    fn drop(&mut self) {
        if let Err(e) = self.shutdown() {
            log::error!("Couldn't shutdown the NGX system {self:?}: {e}");
        }
    }
}

/// An NGX handle. Handle might be created and used by [`Feature::create`].
#[derive(Debug)]
struct FeatureHandle(*mut bindings::NVSDK_NGX_Handle);

impl FeatureHandle {
    fn new(raw_handle: *mut bindings::NVSDK_NGX_Handle) -> Self {
        Self(raw_handle)
    }

    fn release(&self) -> Result {
        unsafe { bindings::NVSDK_NGX_VULKAN_ReleaseFeature(self.0) }.into()
    }
}

impl Drop for FeatureHandle {
    fn drop(&mut self) {
        if let Err(e) = self.release() {
            log::error!("Couldn't release the feature handle: {:?}: {e}", self)
        }
    }
}

/// A type alias for feature parameter, like
/// [`bindings::NVSDK_NGX_Parameter_NumFrames`].
// pub type FeatureParameterName = std::ffi::CStr;
pub type FeatureParameterName = [u8];

macro_rules! insert_parameter_debug {
    ($map:ident, $parameters:ident, ($key:path, bool),) => {
        if let Ok(value) = $parameters.get_bool($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value)
                );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, i32),) => {
        if let Ok(value) = $parameters.get_i32($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, u32),) => {
        if let Ok(value) = $parameters.get_u32($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, f32),) => {
        if let Ok(value) = $parameters.get_f32($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, u64),) => {
        if let Ok(value) = $parameters.get_u64($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, f64),) => {
        if let Ok(value) = $parameters.get_f64($key) {
            $map.insert(
                stringify!($key).to_owned(),
                format!("{:?}", value),
            );
        }
    };
    ($map:ident, $parameters:ident, ($key:path, $typ:ident), $(($next_key:path, $next_type:ident)),+,) => {
        insert_parameter_debug!($map, $parameters, ($key, $typ),);
        insert_parameter_debug!($map, $parameters, $(($next_key, $next_type)),+,);
    };
}

/// Feature parameters is a collection of parameters of a feature (ha!).
// #[derive(Debug)]
pub struct FeatureParameters(*mut bindings::NVSDK_NGX_Parameter);

impl std::fmt::Debug for FeatureParameters {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        struct FeatureParametersDebugPrinter<'a>(&'a FeatureParameters);

        impl<'a> std::fmt::Debug for FeatureParametersDebugPrinter<'a> {
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                use std::collections::HashMap;

                let mut fmt = fmt.debug_struct("FeatureParameters");
                fmt.field("pointer_address", &self.0 .0);

                let populate_map = || -> HashMap<String, String> {
                    let mut map = HashMap::new();
                    let parameters = self.0;

                    // TODO: add more
                    insert_parameter_debug!(
                        map,
                        parameters,
                        (
                            crate::bindings::NVSDK_NGX_Parameter_SuperSampling_Available,
                            bool
                        ),
                        (
                            crate::bindings::NVSDK_NGX_Parameter_InPainting_Available,
                            bool
                        ),
                        (
                            crate::bindings::NVSDK_NGX_Parameter_ImageSuperResolution_Available,
                            bool
                        ),
                        (
                            crate::bindings::NVSDK_NGX_Parameter_SlowMotion_Available,
                            bool
                        ),
                        (
                            crate::bindings::NVSDK_NGX_Parameter_VideoSuperResolution_Available,
                            bool
                        ),
                        (
                            crate::bindings::NVSDK_NGX_Parameter_ImageSignalProcessing_Available,
                            bool
                        ),
                        (crate::bindings::NVSDK_NGX_Parameter_DeepResolve_Available, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_DeepDVC_Available, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_SuperSampling_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_InPainting_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_ImageSuperResolution_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_SlowMotion_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_VideoSuperResolution_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_ImageSignalProcessing_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_DeepResolve_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_DeepDVC_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_FrameInterpolation_NeedsUpdatedDriver, bool),
                        (crate::bindings::NVSDK_NGX_Parameter_NumFrames, u32),
                        (crate::bindings::NVSDK_NGX_Parameter_Scale, u32),
                        (crate::bindings::NVSDK_NGX_Parameter_OptLevel, u32),
                        (
                            crate::bindings::NVSDK_NGX_Parameter_IsDevSnippetBranch,
                            bool
                        ),
                        (
                            crate::bindings::NVSDK_NGX_Parameter_SuperSampling_ScaleFactor,
                            f32
                        ),
                    );
                    map
                };
                let map = populate_map();
                fmt.field("parameters", &map).finish()
            }
        }

        let debug = FeatureParametersDebugPrinter(self);
        fmt.debug_tuple("FeatureParameters").field(&debug).finish()
    }
}

impl FeatureParameters {
    /// Create a new feature parameter set.
    ///
    /// # NVIDIA documentation
    ///
    /// This interface allows allocating a simple parameter setup using named fields, whose
    /// lifetime the app must manage.
    /// For example one can set width by calling Set(NVSDK_NGX_Parameter_Denoiser_Width,100) or
    /// provide CUDA buffer pointer by calling Set(NVSDK_NGX_Parameter_Denoiser_Color,cudaBuffer)
    /// For more details please see sample code.
    /// Parameter maps output by NVSDK_NGX_AllocateParameters must NOT be freed using
    /// the free/delete operator; to free a parameter map
    /// output by NVSDK_NGX_AllocateParameters, NVSDK_NGX_DestroyParameters should be used.
    /// Unlike with NVSDK_NGX_GetParameters, parameter maps allocated with NVSDK_NGX_AllocateParameters
    /// must be destroyed by the app using NVSDK_NGX_DestroyParameters.
    /// Also unlike with NVSDK_NGX_GetParameters, parameter maps output by NVSDK_NGX_AllocateParameters
    /// do not come pre-populated with NGX capabilities and available features.
    /// To create a new parameter map pre-populated with such information, NVSDK_NGX_GetCapabilityParameters
    /// should be used.
    /// This function may return NVSDK_NGX_Result_FAIL_OutOfDate if an older driver, which
    /// does not support this API call is being used. In such a case, NVSDK_NGX_GetParameters
    /// may be used as a fallback.
    /// This function may only be called after a successful call into NVSDK_NGX_Init.
    pub fn new(&self) -> Result<Self> {
        let mut ptr: *mut bindings::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe { bindings::NVSDK_NGX_VULKAN_AllocateParameters(&mut ptr as *mut _) })
            .map(|_| Self(ptr))
    }

    /// Get a feature parameter set populated with NGX and feature capabilities.
    ///
    /// # NVIDIA documentation
    ///
    /// This interface allows the app to create a new parameter map
    /// pre-populated with NGX capabilities and available features.
    /// The output parameter map can also be used for any purpose
    /// parameter maps output by NVSDK_NGX_AllocateParameters can be used for
    /// but it is not recommended to use NVSDK_NGX_GetCapabilityParameters
    /// unless querying NGX capabilities and available features
    /// due to the overhead associated with pre-populating the parameter map.
    /// Parameter maps output by NVSDK_NGX_GetCapabilityParameters must NOT be freed using
    /// the free/delete operator; to free a parameter map
    /// output by NVSDK_NGX_GetCapabilityParameters, NVSDK_NGX_DestroyParameters should be used.
    /// Unlike with NVSDK_NGX_GetParameters, parameter maps allocated with NVSDK_NGX_GetCapabilityParameters
    /// must be destroyed by the app using NVSDK_NGX_DestroyParameters.
    /// This function may return NVSDK_NGX_Result_FAIL_OutOfDate if an older driver, which
    /// does not support this API call is being used. This function may only be called
    /// after a successful call into NVSDK_NGX_Init.
    /// If NVSDK_NGX_GetCapabilityParameters fails with NVSDK_NGX_Result_FAIL_OutOfDate,
    /// NVSDK_NGX_GetParameters may be used as a fallback, to get a parameter map pre-populated
    /// with NGX capabilities and available features.
    pub fn get_capability_parameters() -> Result<Self> {
        let mut ptr: *mut bindings::NVSDK_NGX_Parameter = std::ptr::null_mut();
        Result::from(unsafe {
            bindings::NVSDK_NGX_VULKAN_GetCapabilityParameters(&mut ptr as *mut _)
        })
        .map(|_| Self(ptr))
    }

    /// Sets the value for the parameter named `name` to be a
    /// type-erased (`void *`) pointer.
    pub fn set_ptr<T>(&self, name: &FeatureParameterName, ptr: *mut T) {
        unsafe {
            bindings::NVSDK_NGX_Parameter_SetVoidPointer(
                self.0,
                name.as_ptr().cast(),
                ptr as *mut _,
            );
        }
    }

    /// Returns a type-erased pointer associated with the provided
    /// `name`.
    pub fn get_ptr(&self, name: &FeatureParameterName) -> Result<*mut std::ffi::c_void> {
        let mut ptr = std::ptr::null_mut();
        Result::from(unsafe {
            bindings::NVSDK_NGX_Parameter_GetVoidPointer(
                self.0,
                name.as_ptr().cast(),
                &mut ptr as *mut _,
            )
        })
        .map(|_| ptr)
    }

    /// Sets an [bool] value for the parameter named `name`. The
    /// [bool] type isn't supported in NGX, but the semantics - are. The
    /// boolean values are stored as integers with value `1` being
    /// `true` and `0` being `false`.
    pub fn set_bool(&self, name: &FeatureParameterName, value: bool) {
        unsafe {
            bindings::NVSDK_NGX_Parameter_SetI(
                self.0,
                name.as_ptr().cast(),
                if value { 1 } else { 0 },
            )
        }
    }

    /// Returns a [bool] value of a parameter named `name`.
    /// The [bool] type isn't supported in NGX, but the semantics - are.
    /// The boolean values are stored as integers with value `1` being
    /// `true` and `0` being `false`.
    pub fn get_bool(&self, name: &FeatureParameterName) -> Result<bool> {
        let mut value = 0i32;
        Result::from(unsafe {
            bindings::NVSDK_NGX_Parameter_GetI(self.0, name.as_ptr().cast(), &mut value as *mut _)
        })
        .map(|_| if value == 1 { true } else { false })
    }

    /// Sets an [f32] value for the parameter named `name`.
    pub fn set_f32(&self, name: &FeatureParameterName, value: f32) {
        unsafe { bindings::NVSDK_NGX_Parameter_SetF(self.0, name.as_ptr().cast(), value) }
    }

    /// Returns a [f32] value of a parameter named `name`.
    pub fn get_f32(&self, name: &FeatureParameterName) -> Result<f32> {
        let mut value = 0f32;
        Result::from(unsafe {
            bindings::NVSDK_NGX_Parameter_GetF(self.0, name.as_ptr().cast(), &mut value as *mut _)
        })
        .map(|_| value)
    }

    /// Sets an [u32] value for the parameter named `name`.
    pub fn set_u32(&self, name: &FeatureParameterName, value: u32) {
        unsafe { bindings::NVSDK_NGX_Parameter_SetUI(self.0, name.as_ptr().cast(), value) }
    }

    /// Returns a [u32] value of a parameter named `name`.
    pub fn get_u32(&self, name: &FeatureParameterName) -> Result<u32> {
        let mut value = 0u32;
        Result::from(unsafe {
            bindings::NVSDK_NGX_Parameter_GetUI(self.0, name.as_ptr().cast(), &mut value as *mut _)
        })
        .map(|_| value)
    }

    /// Sets an [f64] value for the parameter named `name`.
    pub fn set_f64(&self, name: &FeatureParameterName, value: f64) {
        unsafe { bindings::NVSDK_NGX_Parameter_SetD(self.0, name.as_ptr().cast(), value) }
    }

    /// Returns a [f64] value of a parameter named `name`.
    pub fn get_f64(&self, name: &FeatureParameterName) -> Result<f64> {
        let mut value = 0f64;
        Result::from(unsafe {
            bindings::NVSDK_NGX_Parameter_GetD(self.0, name.as_ptr().cast(), &mut value as *mut _)
        })
        .map(|_| value)
    }

    /// Sets an [i32] value for the parameter named `name`.
    pub fn set_i32(&self, name: &FeatureParameterName, value: i32) {
        unsafe { bindings::NVSDK_NGX_Parameter_SetI(self.0, name.as_ptr().cast(), value) }
    }

    /// Returns a [i32] value of a parameter named `name`.
    pub fn get_i32(&self, name: &FeatureParameterName) -> Result<i32> {
        let mut value = 0i32;
        Result::from(unsafe {
            bindings::NVSDK_NGX_Parameter_GetI(self.0, name.as_ptr().cast(), &mut value as *mut _)
        })
        .map(|_| value)
    }

    /// Sets an [u64] value for the parameter named `name`.
    pub fn set_u64(&self, name: &FeatureParameterName, value: u64) {
        unsafe { bindings::NVSDK_NGX_Parameter_SetULL(self.0, name.as_ptr().cast(), value) }
    }

    /// Returns a [u64] value of a parameter named `name`.
    pub fn get_u64(&self, name: &FeatureParameterName) -> Result<u64> {
        let mut value = 0u64;
        Result::from(unsafe {
            bindings::NVSDK_NGX_Parameter_GetULL(self.0, name.as_ptr().cast(), &mut value as *mut _)
        })
        .map(|_| value)
    }

    /// Returns `Ok` if the parameters claim to support the
    /// super sampling feature ([`bindings::NVSDK_NGX_Parameter_SuperSampling_Available`]).
    pub fn supports_super_sampling(&self) -> Result<()> {
        if self.get_bool(bindings::NVSDK_NGX_Parameter_SuperSampling_NeedsUpdatedDriver)? {
            let major =
                self.get_u32(bindings::NVSDK_NGX_Parameter_SuperSampling_MinDriverVersionMajor)?;
            let minor =
                self.get_u32(bindings::NVSDK_NGX_Parameter_SuperSampling_MinDriverVersionMinor)?;
            return Err(crate::Error::Other(format!("The SuperSampling feature requires a driver update. The driver version required should be higher or equal to {major}.{minor}")));
        }
        match self.get_bool(bindings::NVSDK_NGX_Parameter_SuperSampling_Available) {
            Ok(true) => Ok(()),
            Ok(false) => Err(crate::Error::Other(format!(
                "The SuperSampling isn't supported on this platform."
            ))),
            Err(e) => Err(e),
        }
    }

    /// Returns `Ok` if the parameters claim to support the
    /// super sampling feature ([`bindings::NVSDK_NGX_Parameter_SuperSampling_Available`]).
    pub fn supports_super_sampling_static() -> Result<()> {
        Self::get_capability_parameters()?.supports_super_sampling()
    }

    /// Deallocates the feature parameter set.
    fn release(&self) -> Result {
        unsafe { bindings::NVSDK_NGX_VULKAN_DestroyParameters(self.0) }.into()
    }
}

impl Drop for FeatureParameters {
    fn drop(&mut self) {
        if let Err(e) = self.release() {
            log::error!(
                "Couldn't release the feature parameter set: {:?}: {e}",
                self
            )
        }
    }
}

/// Describes a single NGX feature.
#[derive(Debug)]
pub struct Feature {
    handle: FeatureHandle,
    feature_type: bindings::NVSDK_NGX_Feature,
    parameters: FeatureParameters,
}

impl Feature {
    /// Creates a new feature.
    pub fn new(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        feature_type: bindings::NVSDK_NGX_Feature,
        parameters: FeatureParameters,
    ) -> Result<Self> {
        let mut handle = FeatureHandle::new(std::ptr::null_mut());
        Result::from(unsafe {
            bindings::NVSDK_NGX_VULKAN_CreateFeature1(
                device.to_pointer_mut(),
                command_buffer.to_pointer_mut(),
                feature_type,
                parameters.0,
                &mut handle.0 as *mut _,
            )
        })
        .map(|_| Self {
            handle,
            feature_type,
            parameters,
        })
    }

    /// Returns the parameters associated with this feature.
    pub fn get_parameters(&self) -> &FeatureParameters {
        &self.parameters
    }

    /// Returns the type of this feature.
    pub fn get_feature_type(&self) -> bindings::NVSDK_NGX_Feature {
        self.feature_type
    }

    /// Returns the number of bytes needed for the scratch buffer for
    /// this feature.
    ///
    /// # NVIDIA documentation
    ///
    /// SDK needs a buffer of a certain size provided by the client in
    /// order to initialize AI feature. Once feature is no longer
    /// needed buffer can be released. It is safe to reuse the same
    /// scratch buffer for different features as long as minimum size
    /// requirement is met for all features. Please note that some
    /// features might not need a scratch buffer so return size of 0
    /// is completely valid.
    pub fn get_scratch_buffer_size(&self) -> Result<usize> {
        let mut size = 0usize;
        Result::from(unsafe {
            bindings::NVSDK_NGX_VULKAN_GetScratchBufferSize(
                self.feature_type,
                self.parameters.0 as _,
                &mut size as *mut _,
            )
        })
        .map(|_| size)
    }

    /// Evalutes the feature.
    ///
    /// # NVIDIA documentation
    ///
    /// Evaluates given feature using the provided parameters and
    /// pre-trained NN. Please note that for most features
    /// it can be benefitials to pass as many input buffers and parameters
    /// as possible (for example provide all render targets like color,
    /// albedo, normals, depth etc)
    pub fn evaluate(&self, command_buffer: vk::CommandBuffer) -> Result {
        unsafe {
            bindings::NVSDK_NGX_VULKAN_EvaluateFeature_C(
                command_buffer.to_pointer_mut(),
                self.handle.0,
                self.parameters.0,
                None,
            )
        }
        .into()
    }
}

/// Describes a set of NGX feature requirements.
#[derive(Debug)]
pub struct FeatureRequirement(bindings::NVSDK_NGX_FeatureRequirement);

/// A helpful type alias to quickly mention "DLSS".
pub type DlssFeature = SuperSamplingFeature;

/// A SuperSamling (or "DLSS") feature.
#[derive(Debug)]
pub struct SuperSamplingFeature(Feature);

impl SuperSamplingFeature {
    // /// Attempts to create the [`SuperSamplingFeature`] with the default
    // /// settings preset.
    // pub fn try_default() -> Result<Self> {
    //     let parameters = FeatureParameters::get_capability_parameters()?;
    //     Self::new(parameters)
    // }

    // /// Creates a new [`SuperSamplingFeature`] with the given
    // /// parameters.
    // pub fn from_feature_unchecked(parameters: FeatureParameters) -> Result<Self> {}

    /// Creates a new [`SuperSamplingFeature`] from the given
    /// [`Feature`] object.
    ///
    /// # Safety
    ///
    /// The function is unsafe due to not checking the actual feature
    /// type within the [`Feature`] provided.
    ///
    /// For the safe and checked version use [`std::convert::TryFrom`].
    pub unsafe fn from_feature_unchecked(feature: Feature) -> Self {
        Self(feature)
    }
}

impl TryFrom<Feature> for SuperSamplingFeature {
    type Error = crate::Error;

    fn try_from(feature: Feature) -> Result<Self> {
        if feature.feature_type == bindings::NVSDK_NGX_Feature::NVSDK_NGX_Feature_SuperSampling {
            Ok(unsafe { Self::from_feature_unchecked(feature) })
        } else {
            Err(crate::Error::Other(format!(
                "The provided Feature isn't of SuperSampling type."
            )))
        }
    }
}

// #[derive(Debug)]
// pub struct FeatureCommonInfo {
//     path_list_info:,

// }

// /// Contains information common to all features, used by NGX in
// /// determining requested feature availability.
// #[derive(Debug, Clone)]
// pub struct FeatureDiscoveryBuilder {
//     /// API Struct version number.
//     sdk_version: Option<bindings::NVSDK_NGX_Version>,
//     /// Valid NVSDK_NGX_Feature enum corresponding to DLSS v3 Feature
//     /// which is being queried for availability.
//     feature_type: Option<bindings::NVSDK_NGX_Feature>,
//     /// Unique Id provided by NVIDIA corresponding to a particular
//     /// Application or alternatively custom Id set by Engine.
//     application_identifier: Option<bindings::NVSDK_NGX_Application_Identifier>,
//     /// Folder to store logs and other temporary files (write access
//     /// required), normally this would be a location in Documents or
//     /// ProgramData.
//     application_data_path: Option<widestring::WideCString>,
//     /// Contains information common to all features, presently only a
//     /// list of all paths feature dlls can be located in, other than the
//     /// default path - application directory.
//     common_info: Option<FeatureCommonInfo>,
// }

// impl FeatureDiscoveryBuilder {
//     /// Creates a new feature discovery builder. The created feature
//     /// discovery builder contains blanket values.
//     pub fn new() -> Self {
//         Self(bindings::NVSDK_NGX_FeatureDiscoveryInfo {
//             SDKVersion: bindings::NVSDK_NGX_Version::NVSDK_NGX_Version_API,
//             FeatureID: bindings::NVSDK_NGX_Feature::NVSDK_NGX_Feature_Reserved_Unknown,

//         })
//     }

//     /// Consumes the builder and obtains the requirements for the
//     /// requested feature based on the information provided.
//     pub fn get_requirements(self) -> Result<FeatureRequirement> {
//         unimplemented!()
//     }
// }

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    #[test]
    fn features() {
        // TODO: initialise vulkan and be able to do this.
        // dbg!(super::FeatureParameters::get_capability_parameters().unwrap());
    }

    #[test]
    fn get_required_extensions() {
        assert!(super::RequiredExtensions::get().is_ok());
    }

    /// Ignored as it just needs to compile.
    #[test]
    #[ignore]
    fn insert_parameter_debug_macro() -> super::Result {
        let mut map = HashMap::new();
        let parameters = super::FeatureParameters::get_capability_parameters().unwrap();
        insert_parameter_debug!(
            map,
            parameters,
            (crate::bindings::NVSDK_NGX_EParameter_Reserved00, i32),
            (
                crate::bindings::NVSDK_NGX_EParameter_SuperSampling_Available,
                bool
            ),
            (
                crate::bindings::NVSDK_NGX_EParameter_InPainting_Available,
                bool
            ),
            (
                crate::bindings::NVSDK_NGX_EParameter_ImageSuperResolution_Available,
                bool
            ),
        );

        Ok(())
    }
}
