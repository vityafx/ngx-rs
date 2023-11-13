//! Vulkan NGX.

#![deny(missing_docs)]

use std::mem::ManuallyDrop;
use std::rc::Rc;

use crate::bindings::{
    self, NVSDK_NGX_DLSS_Create_Params, NVSDK_NGX_DLSS_Feature_Flags, NVSDK_NGX_Dimensions,
    NVSDK_NGX_Feature, NVSDK_NGX_ImageViewInfo_VK, NVSDK_NGX_PerfQuality_Value,
    NVSDK_NGX_Resource_VK_Type, NVSDK_NGX_Resource_VK__bindgen_ty_1, NVSDK_NGX_VK_DLSS_Eval_Params,
    VkFormat, VkImageSubresourceRange,
};
use crate::bindings::{NVSDK_NGX_Coordinates, NVSDK_NGX_Resource_VK};
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
        return Err("Couldn't convert the extensions to CStrings.".into());
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
    unsafe fn as_pointer_mut(&self) -> *mut T;
}

impl<T, H> HandleToPointer<T> for H
where
    H: Handle + Copy,
{
    unsafe fn as_pointer_mut(&self) -> *mut T {
        ash_handle_to_pointer_mut(self)
    }
}

/// NVIDIA NGX system.
#[repr(transparent)]
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
    ASH_ENTRY.as_ref().and_then(|e| {
        let instance = instance as *mut u8;
        let address = instance.offset_from(std::ptr::null::<u8>());
        let raw_handle = address as u64;
        e.get_instance_proc_addr(vk::Instance::from_raw(raw_handle), proc_name)
            .map(|p| std::mem::transmute(p))
    })
}

unsafe extern "C" fn get_device_proc_addr<T>(
    logical_device: *mut T,
    proc_name: *const i8,
) -> Option<unsafe extern "C" fn()> {
    ASH_INSTANCE.as_ref().and_then(|i| {
        let logical_device = logical_device as *mut u8;
        let address = logical_device.offset_from(std::ptr::null::<u8>());
        let raw_handle = address as u64;
        (i.fp_v1_0().get_device_proc_addr)(vk::Device::from_raw(raw_handle), proc_name)
            .map(|p| std::mem::transmute(p))
    })
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
        let project_id =
            std::ffi::CString::new(project_id.unwrap_or_else(uuid::Uuid::new_v4).to_string())
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
                instance.handle().as_pointer_mut(),
                physical_device.as_pointer_mut(),
                logical_device.as_pointer_mut(),
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
        unsafe { bindings::NVSDK_NGX_VULKAN_Shutdown1(self.device.as_pointer_mut()) }.into()
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

    /// Creates a supersampling (or "DLSS") feature.
    pub fn create_super_sampling_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_parameters: FeatureParameters,
        create_parameters: SuperSamplingCreateParameters,
    ) -> Result<SuperSamplingFeature> {
        Feature::new_super_sampling(
            self.device,
            command_buffer,
            feature_parameters,
            create_parameters,
        )
    }

    /// Creates a frame generation feature.
    pub fn create_frame_generation_feature(
        &self,
        command_buffer: vk::CommandBuffer,
        feature_parameters: FeatureParameters,
    ) -> Result<Feature> {
        Feature::new_frame_generation(self.device, command_buffer, feature_parameters)
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
#[repr(transparent)]
#[derive(Debug)]
struct FeatureHandle(*mut bindings::NVSDK_NGX_Handle);

impl Default for FeatureHandle {
    fn default() -> Self {
        Self(std::ptr::null_mut())
    }
}

impl FeatureHandle {
    fn new() -> Self {
        Self::default()
    }

    fn release(&mut self) -> Result {
        unsafe { bindings::NVSDK_NGX_VULKAN_ReleaseFeature(self.0) }.into()
    }
}

impl Drop for FeatureHandle {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }

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
#[repr(transparent)]
pub struct FeatureParameters(*mut bindings::NVSDK_NGX_Parameter);

impl std::fmt::Debug for FeatureParameters {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        #[repr(transparent)]
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
        .map(|_| value == 1)
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
            Ok(false) => Err(crate::Error::Other(
                "The SuperSampling feature isn't supported on this platform.".to_string(),
            )),
            Err(e) => Err(e),
        }
    }

    /// Returns `Ok` if the parameters claim to support the
    /// super sampling feature ([`bindings::NVSDK_NGX_Parameter_SuperSampling_Available`]).
    pub fn supports_super_sampling_static() -> Result<()> {
        Self::get_capability_parameters()?.supports_super_sampling()
    }

    /// Returns `true` if the SuperSampling feature is initialised
    /// correctly.
    pub fn is_super_sampling_initialised(&self) -> bool {
        self.get_bool(bindings::NVSDK_NGX_Parameter_SuperSampling_FeatureInitResult)
            .unwrap_or(false)
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
    handle: Rc<FeatureHandle>,
    feature_type: bindings::NVSDK_NGX_Feature,
    parameters: Rc<FeatureParameters>,
}

impl Feature {
    /// Creates a new feature.
    pub fn new(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        feature_type: bindings::NVSDK_NGX_Feature,
        parameters: FeatureParameters,
    ) -> Result<Self> {
        let mut handle = FeatureHandle::new();
        Result::from(unsafe {
            bindings::NVSDK_NGX_VULKAN_CreateFeature1(
                device.as_pointer_mut(),
                command_buffer.as_pointer_mut(),
                feature_type,
                parameters.0,
                &mut handle.0 as *mut _,
            )
        })
        .map(|_| Self {
            handle: handle.into(),
            feature_type,
            parameters: parameters.into(),
        })
    }

    /// Creates a new SuperSampling feature.
    pub fn new_super_sampling(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        parameters: FeatureParameters,
        mut super_sampling_create_parameters: SuperSamplingCreateParameters,
    ) -> Result<SuperSamplingFeature> {
        let feature_type = NVSDK_NGX_Feature::NVSDK_NGX_Feature_SuperSampling;
        let rendering_resolution = vk::Extent2D::builder()
            .width(super_sampling_create_parameters.0.Feature.InWidth)
            .height(super_sampling_create_parameters.0.Feature.InHeight)
            .build();
        let target_resolution = vk::Extent2D::builder()
            .width(super_sampling_create_parameters.0.Feature.InTargetWidth)
            .height(super_sampling_create_parameters.0.Feature.InTargetHeight)
            .build();
        unsafe {
            let mut handle = FeatureHandle::new();
            Result::from(bindings::HELPERS_NGX_VULKAN_CREATE_DLSS_EXT1(
                device.as_pointer_mut(),
                command_buffer.as_pointer_mut(),
                1,
                1,
                &mut handle.0 as *mut _,
                parameters.0,
                &mut super_sampling_create_parameters.0 as *mut _,
            ))
            .and_then(|_| {
                SuperSamplingFeature::new(
                    Self {
                        handle: handle.into(),
                        feature_type,
                        parameters: parameters.into(),
                    },
                    rendering_resolution,
                    target_resolution,
                )
            })
        }
    }

    /// Creates the Frame Generation feature.
    pub fn new_frame_generation(
        device: vk::Device,
        command_buffer: vk::CommandBuffer,
        parameters: FeatureParameters,
    ) -> Result<Self> {
        let feature_type = NVSDK_NGX_Feature::NVSDK_NGX_Feature_FrameGeneration;
        Self::new(device, command_buffer, feature_type, parameters)
    }

    /// Returns the parameters associated with this feature.
    pub fn get_parameters(&self) -> &FeatureParameters {
        &self.parameters
    }

    /// Returns the parameters associated with this feature.
    pub fn get_parameters_mut(&mut self) -> &mut FeatureParameters {
        Rc::get_mut(&mut self.parameters).unwrap()
    }

    /// Returns the type of this feature.
    pub fn get_feature_type(&self) -> NVSDK_NGX_Feature {
        self.feature_type
    }

    /// Returns [`true`] if this feature is the super sampling one.
    pub fn is_super_sampling(&self) -> bool {
        self.feature_type == NVSDK_NGX_Feature::NVSDK_NGX_Feature_SuperSampling
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
                command_buffer.as_pointer_mut(),
                self.handle.0,
                self.parameters.0,
                Some(feature_progress_callback),
            )
        }
        .into()
    }
}

unsafe extern "C" fn feature_progress_callback(progress: f32, _should_cancel: *mut bool) {
    log::debug!("Feature evalution progress={progress}.");
}

/// Describes a set of NGX feature requirements.
#[repr(transparent)]
#[derive(Debug)]
pub struct FeatureRequirement(bindings::NVSDK_NGX_FeatureRequirement);

/// A helpful type alias to quickly mention "DLSS".
pub type DlssFeature = SuperSamplingFeature;

/// Optimal settings for the DLSS based on the desired quality level and
/// resolution.
#[derive(Debug)]
pub struct SuperSamplingOptimalSettings {
    /// The render width which the renderer must render to before
    /// upscaling.
    pub render_width: u32,
    /// The render height which the renderer must render to before
    /// upscaling.
    pub render_height: u32,
    /// The target width desired, to which the SuperSampling feature
    /// will upscale to.
    pub target_width: u32,
    /// The target height desired, to which the SuperSampling feature
    /// will upscale to.
    pub target_height: u32,
    /// The requested quality level.
    pub desired_quality_level: bindings::NVSDK_NGX_PerfQuality_Value,
    /// TODO:
    pub dynamic_min_render_width: u32,
    /// TODO:
    pub dynamic_max_render_width: u32,
    /// TODO:
    pub dynamic_min_render_height: u32,
    /// TODO:
    pub dynamic_max_render_height: u32,
}

impl SuperSamplingOptimalSettings {
    /// Returns a set of optimal settings for the desired parameter
    /// set, render dimensions and quality level.
    pub fn get_optimal_settings(
        parameters: &FeatureParameters,
        target_width: u32,
        target_height: u32,
        desired_quality_level: bindings::NVSDK_NGX_PerfQuality_Value,
    ) -> Result<Self> {
        let mut settings = Self {
            render_width: 0,
            render_height: 0,
            target_width,
            target_height,
            desired_quality_level,
            dynamic_min_render_width: 0,
            dynamic_max_render_width: 0,
            dynamic_min_render_height: 0,
            dynamic_max_render_height: 0,
        };
        // The sharpness is deprecated, should stay zero.
        let mut sharpness = 0.0f32;
        Result::from(unsafe {
            bindings::HELPERS_NGX_DLSS_GET_OPTIMAL_SETTINGS(
                parameters.0,
                settings.target_width,
                settings.target_height,
                settings.desired_quality_level,
                &mut settings.render_width as *mut _,
                &mut settings.render_height as *mut _,
                &mut settings.dynamic_max_render_width as *mut _,
                &mut settings.dynamic_max_render_height as *mut _,
                &mut settings.dynamic_min_render_width as *mut _,
                &mut settings.dynamic_min_render_height as *mut _,
                &mut sharpness as *mut _,
            )
        })?;

        if settings.render_height == 0 || settings.render_width == 0 {
            return Err(crate::Error::Other(format!(
                "The requested quality level isn't supported: {desired_quality_level:?}"
            )));
        }

        Ok(settings)
    }
}

/// Create parameters for the SuperSampling feature.
#[repr(transparent)]
#[derive(Debug)]
pub struct SuperSamplingCreateParameters(bindings::NVSDK_NGX_DLSS_Create_Params);

impl SuperSamplingCreateParameters {
    /// Creates a new set of create parameters for the SuperSampling
    /// feature.
    pub fn new(
        render_width: u32,
        render_height: u32,
        target_width: u32,
        target_height: u32,
        quality_value: Option<NVSDK_NGX_PerfQuality_Value>,
        flags: Option<NVSDK_NGX_DLSS_Feature_Flags>,
    ) -> Self {
        let mut params: NVSDK_NGX_DLSS_Create_Params = unsafe { std::mem::zeroed() };
        params.Feature.InWidth = render_width;
        params.Feature.InHeight = render_height;
        params.Feature.InTargetWidth = target_width;
        params.Feature.InTargetHeight = target_height;
        if let Some(quality_value) = quality_value {
            params.Feature.InPerfQualityValue = quality_value;
        }
        params.InFeatureCreateFlags = flags.map(|f| f.0).unwrap_or(0);
        Self(params)
    }
}

impl From<SuperSamplingOptimalSettings> for SuperSamplingCreateParameters {
    fn from(value: SuperSamplingOptimalSettings) -> Self {
        Self::new(
            value.render_width,
            value.render_height,
            value.target_width,
            value.target_height,
            Some(value.desired_quality_level),
            Some(
                NVSDK_NGX_DLSS_Feature_Flags::NVSDK_NGX_DLSS_Feature_Flags_AutoExposure
                    | NVSDK_NGX_DLSS_Feature_Flags::NVSDK_NGX_DLSS_Feature_Flags_MVLowRes,
            ),
        )
    }
}

// /// Only mandatory parameters for the SuperSampling feature evaluation.
// #[derive(Debug, derive_builder::Builder)]
// pub struct SuperSamplingEvaluationParametersSimple {
//     /// The feature evaluation parameters, specific to Vulkan.
//     feature_evaluation_parameters: bindings::NVSDK_NGX_VK_Feature_Eval_Params,
//     /// The depth information.
//     depth: bindings::NVSDK_NGX_Resource_VK,
//     /// The motion vectors.
//     motion_vectors: bindings::NVSDK_NGX_Resource_VK,
//     /// Jitter offset x.
//     jitter_offset_x: f32,
//     /// Jitter offset y.
//     jitter_offset_y: f32,
//     /// The dimensions of the viewport.
//     dimensions: bindings::NVSDK_NGX_Dimensions,
// }

// impl From<SuperSamplingEvaluationParametersSimple> for SuperSamplingEvaluationParameters {
//     fn from(value: SuperSamplingEvaluationParametersSimple) -> Self {
//         let mut params: bindings::NVSDK_NGX_VK_DLSS_Eval_Params = unsafe { std::mem::zeroed() };
//         params.Feature = value.feature_evaluation_parameters;
//         params.pInDepth = value.depth;
//         unsafe {
//             bindings::HELPERS_NVSDK_NGX_Create_ImageView_Resource_VK(imageView, image, subresourceRange, format, width, height, readWrite)
//         }
//         Self(params)
//     }
// }

/// A mode that a vulkan resource might have.
#[derive(Default, Debug, Copy, Clone)]
pub enum VkResourceMode {
    /// Indicates that the resource can only be read.
    #[default]
    Readable,
    /// Indicates that the resource can be written to.
    Writable,
}

/// A struct, objects of which should be hold by
/// [`SuperSamplingEvaluationParameters`] for feature evaluation.
#[derive(Debug, Default, Copy, Clone)]
pub struct VkBufferResourceDescription {
    /// The buffer!
    pub buffer: vk::Buffer,
    /// The size of the buffer in bytes.
    pub size_in_bytes: usize,
    /// The mode this resource has.
    pub mode: VkResourceMode,
}

/// A struct, objects of which should be hold by
/// [`SuperSamplingEvaluationParameters`] for feature evaluation.
#[derive(Debug, Default, Copy, Clone)]
pub struct VkImageResourceDescription {
    /// The image view.
    pub image_view: vk::ImageView,
    /// The image.
    pub image: vk::Image,
    /// The subresource range.
    pub subresource_range: vk::ImageSubresourceRange,
    /// The format.
    pub format: vk::Format,
    /// The width of the image.
    pub width: u32,
    /// The height of the image.
    pub height: u32,
    /// The mode this resource has.
    pub mode: VkResourceMode,
}

impl VkImageResourceDescription {
    /// Sets the writable bit.
    pub fn set_writable(&mut self) {
        self.mode = VkResourceMode::Writable;
    }
}

impl From<VkImageResourceDescription> for NVSDK_NGX_Resource_VK {
    fn from(value: VkImageResourceDescription) -> Self {
        let vk_image_subresource_range = VkImageSubresourceRange {
            aspectMask: value.subresource_range.aspect_mask.as_raw(),
            baseMipLevel: value.subresource_range.base_mip_level,
            baseArrayLayer: value.subresource_range.base_array_layer,
            levelCount: value.subresource_range.level_count,
            layerCount: value.subresource_range.layer_count,
        };
        let mut vk_format: VkFormat = unsafe { std::mem::zeroed() };
        unsafe {
            let ptr = &mut vk_format as *mut _ as *mut i32;
            *ptr = value.format.as_raw();
        }
        let image_resource = NVSDK_NGX_Resource_VK__bindgen_ty_1 {
            ImageViewInfo: NVSDK_NGX_ImageViewInfo_VK {
                ImageView: unsafe { value.image_view.as_pointer_mut() },
                Image: unsafe { value.image.as_pointer_mut() },
                SubresourceRange: vk_image_subresource_range,
                Format: vk_format,
                Width: value.width,
                Height: value.height,
            },
        };

        Self {
            Resource: image_resource,
            Type: NVSDK_NGX_Resource_VK_Type::NVSDK_NGX_RESOURCE_VK_TYPE_VK_IMAGEVIEW,
            ReadWrite: matches!(value.mode, VkResourceMode::Writable),
        }
    }
}

/// The SuperSampling evaluation parameters.
#[derive(Debug)]
pub struct SuperSamplingEvaluationParameters {
    /// The vulkan resource which is an input to the evaluation
    /// parameters (for the upscaling).
    input_color_resource: NVSDK_NGX_Resource_VK,
    /// The vulkan resource which is the output of the evaluation,
    /// so the upscaled image.
    output_color_resource: NVSDK_NGX_Resource_VK,
    /// The depth buffer.
    depth_resource: NVSDK_NGX_Resource_VK,
    /// The motion vectors.
    motion_vectors_resource: NVSDK_NGX_Resource_VK,

    /// This member isn't visible, as it shouldn't be managed by
    /// the user of this struct. Instead, this struct provides an
    /// interface that populates this object and keeps it well-
    /// maintained.
    parameters: NVSDK_NGX_VK_DLSS_Eval_Params,
}

impl Default for SuperSamplingEvaluationParameters {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl SuperSamplingEvaluationParameters {
    /// Creates a new set of evaluation parameters for SuperSampling.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the color input parameter (the image to upscale).
    pub fn set_color_input(&mut self, description: VkImageResourceDescription) {
        self.input_color_resource = description.into();
        self.parameters.Feature.pInColor = std::ptr::addr_of_mut!(self.input_color_resource);
    }

    /// Sets the color output (the upscaled image) information.
    pub fn set_color_output(&mut self, description: VkImageResourceDescription) {
        self.output_color_resource = description.into();
        self.parameters.Feature.pInOutput = std::ptr::addr_of_mut!(self.output_color_resource);
    }

    /// Sets the motion vectors.
    /// In case the `scale` argument is omitted, the `1.0f32` scaling is
    /// used.
    pub fn set_motions_vectors(
        &mut self,
        description: VkImageResourceDescription,
        scale: Option<[f32; 2]>,
    ) {
        const DEFAULT_SCALING: [f32; 2] = [1.0f32, 1.0f32];

        self.motion_vectors_resource = description.into();
        let scales = scale.unwrap_or(DEFAULT_SCALING);
        self.parameters.pInMotionVectors = std::ptr::addr_of_mut!(self.motion_vectors_resource);
        self.parameters.InMVScaleX = scales[0];
        self.parameters.InMVScaleY = scales[1];
    }

    /// Sets the depth buffer.
    pub fn set_depth_buffer(&mut self, description: VkImageResourceDescription) {
        self.depth_resource = description.into();
        self.parameters.pInDepth = std::ptr::addr_of_mut!(self.depth_resource);
    }

    /// Sets the jitter offsets (like TAA).
    pub fn set_jitter_offsets(&mut self, x: f32, y: f32) {
        self.parameters.InJitterOffsetX = x;
        self.parameters.InJitterOffsetY = y;
    }

    /// Sets/unsets the reset flag.
    pub fn set_reset(&mut self, should_reset: bool) {
        self.parameters.InReset = if should_reset { 1 } else { 0 };
    }

    /// Sets the rendering dimensions.
    pub fn set_rendering_dimensions(
        &mut self,
        rendering_offset: [u32; 2],
        rendering_size: [u32; 2],
    ) {
        self.parameters.InColorSubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InDepthSubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InTranslucencySubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InMVSubrectBase = NVSDK_NGX_Coordinates {
            X: rendering_offset[0],
            Y: rendering_offset[1],
        };
        self.parameters.InRenderSubrectDimensions = NVSDK_NGX_Dimensions {
            Width: rendering_size[0],
            Height: rendering_size[1],
        };
    }

    /// Returns the filled DLSS parameters.
    pub(crate) fn get_dlss_evaluation_parameters(
        &mut self,
    ) -> *mut bindings::NVSDK_NGX_VK_DLSS_Eval_Params {
        std::ptr::addr_of_mut!(self.parameters)
    }

    // /// Returns an immutable reference to the color output.
    // pub fn get_color_output(&self) -> &VkImageResourceDescription {
    //     &self.color_output
    // }

    // /// Returns a mutable reference to the color output.
    // pub fn get_color_output_mut(&mut self) -> &mut VkImageResourceDescription {
    //     &mut self.color_output
    // }

    // /// Returns an immutable reference to the depth.
    // pub fn get_color(&self) -> &VkBufferResourceDescription {
    //     &self.depth
    // }

    // /// Returns a mutable reference to the depth.
    // pub fn get_color_mut(&mut self) -> &mut VkBufferResourceDescription {
    //     &mut self.depth
    // }
}

/// A SuperSamling (or "DLSS") feature.
#[derive(Debug)]
pub struct SuperSamplingFeature {
    feature: Feature,
    parameters: SuperSamplingEvaluationParameters,
    rendering_resolution: vk::Extent2D,
    target_resolution: vk::Extent2D,
}

impl SuperSamplingFeature {
    /// Creates a new Super Sampling feature.
    pub fn new(
        feature: Feature,
        rendering_resolution: vk::Extent2D,
        target_resolution: vk::Extent2D,
    ) -> Result<Self> {
        if !feature.is_super_sampling() {
            return Err(crate::error::Error::Other(
                "Attempt to create a super sampling feature with another feature.".to_owned(),
            ));
        }

        Ok(Self {
            feature,
            parameters: SuperSamplingEvaluationParameters::new(),
            rendering_resolution,
            target_resolution,
        })
    }

    /// Returns the inner feature object.
    pub fn get_inner(&self) -> &Feature {
        &self.feature
    }

    /// Returns the inner feature object (mutable).
    pub fn get_inner_mut(&mut self) -> &mut Feature {
        &mut self.feature
    }

    /// Returns the rendering resolution (input resolution) of the
    /// image that needs to be upscaled to the [`Self::target_resolution`].
    pub const fn get_rendering_resolution(&self) -> vk::Extent2D {
        self.rendering_resolution
    }

    /// Returns the target resolution (output resolution) of the
    /// image that the original image should be upscaled to.
    pub const fn get_target_resolution(&self) -> vk::Extent2D {
        self.target_resolution
    }

    // /// Attempts to create the [`SuperSamplingFeature`] with the default
    // /// settings preset.
    // pub fn try_default() -> Result<Self> {
    //     let parameters = FeatureParameters::get_capability_parameters()?;
    //     Self::new(parameters)
    // }

    /// See [`FeatureParameters::is_super_sampling_initialised`].
    pub fn is_initialised(&self) -> bool {
        self.feature
            .get_parameters()
            .is_super_sampling_initialised()
    }

    /// Returns the evaluation parameters.
    pub fn get_evaluation_parameters_mut(&mut self) -> &mut SuperSamplingEvaluationParameters {
        &mut self.parameters
    }

    /// Evaluates the feature.
    pub fn evaluate(&mut self, command_buffer: vk::CommandBuffer) -> Result {
        Result::from(unsafe {
            bindings::HELPERS_NGX_VULKAN_EVALUATE_DLSS_EXT(
                command_buffer.as_pointer_mut(),
                self.feature.handle.0,
                self.feature.parameters.0,
                self.parameters.get_dlss_evaluation_parameters(),
            )
        })
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
