//! Minimal COM/WMI client.
//!
//! Only the pieces the app needs are implemented: connect to a namespace, read
//! properties and invoke the Dell `AWCCWmiMethodFunction` methods. Interface
//! vtable layouts follow `wbemcli.h` (IWbemLocator, IWbemServices,
//! IWbemClassObject, IEnumWbemClassObject).

#![allow(non_snake_case, clippy::too_many_arguments)]

use std::ffi::c_void;
use std::fmt;
use std::ptr::{null, null_mut};

use crate::win32::*;

pub const CLSID_WBEM_LOCATOR: GUID = GUID {
    data1: 0x4590_F811,
    data2: 0x1D3A,
    data3: 0x11D0,
    data4: [0x89, 0x1F, 0x00, 0xAA, 0x00, 0x4B, 0x2E, 0x24],
};

pub const IID_IWBEM_LOCATOR: GUID = GUID {
    data1: 0xDC12_A687,
    data2: 0x737F,
    data3: 0x11CF,
    data4: [0x88, 0x4D, 0x00, 0xAA, 0x00, 0x4B, 0x2E, 0x24],
};

const WBEM_FLAG_RETURN_IMMEDIATELY: i32 = 0x10;
const WBEM_FLAG_FORWARD_ONLY: i32 = 0x20;
const WBEM_FLAG_NONSYSTEM_ONLY: i32 = 0x40;
const WBEM_INFINITE: i32 = -1;

const RPC_E_TOO_LATE: HRESULT = 0x8001_0119u32 as HRESULT;

type Hr = HRESULT;

// ------------------------------------------------------------ error handling

#[derive(Debug)]
pub struct WmiError {
    pub context: &'static str,
    pub hr: HRESULT,
}

impl WmiError {
    fn new(context: &'static str, hr: HRESULT) -> Self {
        WmiError { context, hr }
    }
}

impl fmt::Display for WmiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed ({})", self.context, hresult_message(self.hr))
    }
}

impl std::error::Error for WmiError {}

// ------------------------------------------------------------------- COM core

/// Calls `Release` on the interface pointer it owns.
struct ComPtr {
    ptr: *mut c_void,
}

impl ComPtr {
    /// # Safety
    /// `ptr` must be a valid COM interface pointer the caller gives ownership of.
    unsafe fn from_raw(ptr: *mut c_void) -> Self {
        ComPtr { ptr }
    }

    fn as_raw(&self) -> *mut c_void {
        self.ptr
    }

    fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl Drop for ComPtr {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { release(self.ptr) };
        }
    }
}

unsafe fn vtable<T>(ptr: *mut c_void) -> *const T {
    *(ptr as *const *const T)
}

unsafe fn release(ptr: *mut c_void) {
    #[repr(C)]
    struct Vtbl {
        query_interface: *const c_void,
        add_ref: *const c_void,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }
    let vt = vtable::<Vtbl>(ptr);
    ((*vt).release)(ptr);
}

/// COM apartment guard. `CoUninitialize` runs on drop.
pub struct Com {
    _private: (),
}

impl Com {
    /// `apartment` selects an STA (`true`) or MTA (`false`) apartment.
    pub fn init(apartment: bool) -> Result<Com, WmiError> {
        let flags = if apartment {
            COINIT_APARTMENTTHREADED
        } else {
            COINIT_MULTITHREADED
        };
        let hr = unsafe { CoInitializeEx(null_mut(), flags) };
        // S_FALSE means "already initialized", which is fine.
        if hr < 0 {
            return Err(WmiError::new("CoInitializeEx", hr));
        }
        let hr = unsafe {
            CoInitializeSecurity(
                null_mut(),
                -1,
                null_mut(),
                null_mut(),
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                null_mut(),
                EOAC_NONE,
                null_mut(),
            )
        };
        if hr < 0 && hr != RPC_E_TOO_LATE {
            return Err(WmiError::new("CoInitializeSecurity", hr));
        }
        Ok(Com { _private: () })
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// Owned BSTR.
struct Bstr {
    ptr: BSTR,
}

impl Bstr {
    fn new(s: &str) -> Bstr {
        let wide = to_wide(s);
        // Exclude the terminating NUL: `SysAllocStringLen` adds its own.
        let ptr = unsafe { SysAllocStringLen(wide.as_ptr(), (wide.len() - 1) as u32) };
        Bstr { ptr }
    }

    fn as_ptr(&self) -> BSTR {
        self.ptr
    }
}

impl Drop for Bstr {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { SysFreeString(self.ptr) };
        }
    }
}

// ------------------------------------------------------------- WMI interfaces

#[repr(C)]
struct IWbemLocatorVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> Hr,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    connect_server: unsafe extern "system" fn(
        *mut c_void,
        BSTR,
        BSTR,
        BSTR,
        BSTR,
        i32,
        BSTR,
        *mut c_void,
        *mut *mut c_void,
    ) -> Hr,
}

#[repr(C)]
struct IWbemServicesVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> Hr,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    open_namespace: *const c_void,
    cancel_async_call: *const c_void,
    query_object_sink: *const c_void,
    get_object: unsafe extern "system" fn(
        *mut c_void,
        BSTR,
        i32,
        *mut c_void,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> Hr,
    get_object_async: *const c_void,
    put_class: *const c_void,
    put_class_async: *const c_void,
    delete_class: *const c_void,
    delete_class_async: *const c_void,
    create_class_enum: *const c_void,
    create_class_enum_async: *const c_void,
    put_instance: *const c_void,
    put_instance_async: *const c_void,
    delete_instance: *const c_void,
    delete_instance_async: *const c_void,
    create_instance_enum: *const c_void,
    create_instance_enum_async: *const c_void,
    exec_query: unsafe extern "system" fn(
        *mut c_void,
        BSTR,
        BSTR,
        i32,
        *mut c_void,
        *mut *mut c_void,
    ) -> Hr,
    exec_query_async: *const c_void,
    exec_notification_query: *const c_void,
    exec_notification_query_async: *const c_void,
    exec_method: unsafe extern "system" fn(
        *mut c_void,
        BSTR,
        BSTR,
        i32,
        *mut c_void,
        *mut c_void,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> Hr,
    exec_method_async: *const c_void,
}

#[repr(C)]
struct IWbemClassObjectVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> Hr,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_qualifier_set: *const c_void,
    get:
        unsafe extern "system" fn(*mut c_void, PCWSTR, i32, *mut VARIANT, *mut i32, *mut i32) -> Hr,
    put: unsafe extern "system" fn(*mut c_void, PCWSTR, i32, *const VARIANT, i32) -> Hr,
    delete: *const c_void,
    get_names:
        unsafe extern "system" fn(*mut c_void, PCWSTR, i32, *mut VARIANT, *mut *mut c_void) -> Hr,
    begin_enumeration: *const c_void,
    next: *const c_void,
    end_enumeration: *const c_void,
    get_property_qualifier_set: *const c_void,
    clone: *const c_void,
    get_object_text: *const c_void,
    spawn_derived_class: *const c_void,
    spawn_instance: unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> Hr,
    compare_to: *const c_void,
    get_property_origin: *const c_void,
    inherits_from: *const c_void,
    get_method: unsafe extern "system" fn(
        *mut c_void,
        PCWSTR,
        i32,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> Hr,
}

#[repr(C)]
struct IEnumWbemClassObjectVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> Hr,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    reset: *const c_void,
    next: unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void, *mut u32) -> Hr,
    next_async: *const c_void,
    clone: *const c_void,
    skip: *const c_void,
}

// -------------------------------------------------------------- WMI objects

/// A WMI class or instance.
pub struct WmiObject {
    ptr: ComPtr,
}

impl WmiObject {
    fn vtbl(&self) -> &IWbemClassObjectVtbl {
        unsafe { &*vtable::<IWbemClassObjectVtbl>(self.ptr.as_raw()) }
    }

    fn get_variant(&self, name: &str) -> Option<VARIANT> {
        let name = to_wide(name);
        let mut value = VARIANT::empty();
        let hr = unsafe {
            (self.vtbl().get)(
                self.ptr.as_raw(),
                name.as_ptr(),
                0,
                &mut value,
                null_mut(),
                null_mut(),
            )
        };
        if hr < 0 {
            return None;
        }
        Some(value)
    }

    /// Reads a string property.
    pub fn get_string(&self, name: &str) -> Option<String> {
        let mut value = self.get_variant(name)?;
        let result = if value.vt == VT_BSTR && !value.bstr().is_null() {
            let mut len = 0usize;
            unsafe {
                while *value.bstr().add(len) != 0 {
                    len += 1;
                }
                String::from_utf16_lossy(std::slice::from_raw_parts(value.bstr(), len))
            }
        } else {
            return None;
        };
        unsafe { VariantClear(&mut value as *mut VARIANT) };
        Some(result)
    }

    /// Reads an integer property, tolerating any integer flavour.
    pub fn get_u32(&self, name: &str) -> Option<u32> {
        let value = self.get_variant(name)?;
        Some(match value.vt {
            VT_I2 | VT_UI2 => value.data[0] as u32 | ((value.data[1] as u32) << 8),
            VT_I4 | VT_UI4 => value.as_u32(),
            VT_I8 | VT_UI8 => value.as_i64() as u32,
            VT_BOOL => (value.data[0] != 0) as u32,
            _ => return None,
        })
    }

    /// Builds an in-parameter instance from this method signature object.
    fn spawn_instance(&self) -> Option<WmiObject> {
        let mut out: *mut c_void = null_mut();
        let hr = unsafe { (self.vtbl().spawn_instance)(self.ptr.as_raw(), 0, &mut out) };
        if hr < 0 || out.is_null() {
            return None;
        }
        Some(WmiObject {
            ptr: unsafe { ComPtr::from_raw(out) },
        })
    }

    fn put_u32(&self, name: &str, value: u32) -> bool {
        let name = to_wide(name);
        let variant = VARIANT::from_i4(value as i32);
        let hr = unsafe { (self.vtbl().put)(self.ptr.as_raw(), name.as_ptr(), 0, &variant, 0) };
        hr >= 0
    }

    fn get_method(&self, method: &str) -> Option<WmiObject> {
        let name = to_wide(method);
        let mut in_signature: *mut c_void = null_mut();
        let hr = unsafe {
            (self.vtbl().get_method)(
                self.ptr.as_raw(),
                name.as_ptr(),
                0,
                &mut in_signature,
                null_mut(),
            )
        };
        if hr < 0 || in_signature.is_null() {
            return None;
        }
        Some(WmiObject {
            ptr: unsafe { ComPtr::from_raw(in_signature) },
        })
    }

    /// Property names of this object (used to find method parameter names).
    fn get_names(&self) -> Vec<String> {
        let mut array: *mut c_void = null_mut();
        let hr = unsafe {
            (self.vtbl().get_names)(
                self.ptr.as_raw(),
                null(),
                WBEM_FLAG_NONSYSTEM_ONLY,
                null_mut(),
                &mut array,
            )
        };
        if hr < 0 || array.is_null() {
            return Vec::new();
        }

        let mut names = Vec::new();
        unsafe {
            let mut lower: i32 = 0;
            let mut upper: i32 = -1;
            SafeArrayGetLBound(array, 1, &mut lower);
            SafeArrayGetUBound(array, 1, &mut upper);
            for index in lower..=upper {
                let mut name: BSTR = null();
                let ok =
                    SafeArrayGetElement(array, &index, &mut name as *mut BSTR as *mut c_void) >= 0;
                if ok && !name.is_null() {
                    let mut len = 0usize;
                    while *name.add(len) != 0 {
                        len += 1;
                    }
                    names.push(String::from_utf16_lossy(std::slice::from_raw_parts(
                        name, len,
                    )));
                    SysFreeString(name);
                }
            }
            SafeArrayDestroy(array);
        }
        names
    }
}

/// A WMI method that takes a single `uint32` and returns a single `uint32`.
pub struct WmiMethod {
    name: String,
    in_signature: WmiObject,
    /// Name of the single `uint32` input parameter, as declared by the driver.
    argument: String,
}

/// Connection to one WMI namespace.
pub struct Wmi {
    services: ComPtr,
}

impl Wmi {
    pub fn connect(namespace: &str) -> Result<Wmi, WmiError> {
        unsafe {
            let mut locator: *mut c_void = null_mut();
            let hr = CoCreateInstance(
                &CLSID_WBEM_LOCATOR,
                null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_IWBEM_LOCATOR,
                &mut locator,
            );
            if hr < 0 {
                return Err(WmiError::new("CoCreateInstance(WbemLocator)", hr));
            }
            let locator = ComPtr::from_raw(locator);

            let vtbl = &*vtable::<IWbemLocatorVtbl>(locator.as_raw());
            let ns = Bstr::new(namespace);
            let mut services: *mut c_void = null_mut();
            let hr = (vtbl.connect_server)(
                locator.as_raw(),
                ns.as_ptr(),
                null(),
                null(),
                null(),
                0,
                null(),
                null_mut(),
                &mut services,
            );
            if hr < 0 || services.is_null() {
                return Err(WmiError::new("IWbemLocator::ConnectServer", hr));
            }

            // Local connections still need the proxy blanket set.
            let hr = CoSetProxyBlanket(
                services,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                null(),
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                null_mut(),
                EOAC_NONE,
            );
            if hr < 0 {
                release(services);
                return Err(WmiError::new("CoSetProxyBlanket", hr));
            }

            Ok(Wmi {
                services: ComPtr::from_raw(services),
            })
        }
    }

    fn services_vtbl(&self) -> &IWbemServicesVtbl {
        unsafe { &*vtable::<IWbemServicesVtbl>(self.services.as_raw()) }
    }

    /// Fetches a class (or instance) by object path.
    pub fn get_object(&self, path: &str) -> Option<WmiObject> {
        let path = Bstr::new(path);
        let mut out: *mut c_void = null_mut();
        let hr = unsafe {
            (self.services_vtbl().get_object)(
                self.services.as_raw(),
                path.as_ptr(),
                0,
                null_mut(),
                &mut out,
                null_mut(),
            )
        };
        if hr < 0 || out.is_null() {
            return None;
        }
        Some(WmiObject {
            ptr: unsafe { ComPtr::from_raw(out) },
        })
    }

    /// Runs a WQL query and collects every result object.
    pub fn query(&self, wql: &str) -> Option<Vec<WmiObject>> {
        const MAX_OBJECTS: usize = 64;
        let language = Bstr::new("WQL");
        let query = Bstr::new(wql);
        let mut enumerator: *mut c_void = null_mut();
        let hr = unsafe {
            (self.services_vtbl().exec_query)(
                self.services.as_raw(),
                language.as_ptr(),
                query.as_ptr(),
                WBEM_FLAG_RETURN_IMMEDIATELY | WBEM_FLAG_FORWARD_ONLY,
                null_mut(),
                &mut enumerator,
            )
        };
        if hr < 0 || enumerator.is_null() {
            return None;
        }
        let enumerator = unsafe { ComPtr::from_raw(enumerator) };

        let mut objects = Vec::new();
        unsafe {
            let vtbl = &*vtable::<IEnumWbemClassObjectVtbl>(enumerator.as_raw());
            loop {
                let mut object: *mut c_void = null_mut();
                let mut returned: u32 = 0;
                let hr = (vtbl.next)(
                    enumerator.as_raw(),
                    WBEM_INFINITE,
                    1,
                    &mut object,
                    &mut returned,
                );
                if hr < 0 || returned == 0 || object.is_null() {
                    break;
                }
                objects.push(WmiObject {
                    ptr: ComPtr::from_raw(object),
                });
                if objects.len() >= MAX_OBJECTS {
                    break;
                }
            }
        }
        Some(objects)
    }

    /// Looks up the input signature of a method taking one `uint32` argument.
    pub fn method(&self, class_path: &str, method: &str) -> Option<WmiMethod> {
        let class = self.get_object(class_path)?;
        let in_signature = class.get_method(method)?;
        // The parameter name is driver specific (`arg2` on the documented
        // models, something else elsewhere), so read it instead of guessing.
        let argument = in_signature
            .get_names()
            .into_iter()
            .next()
            .unwrap_or_else(|| "arg2".to_string());
        Some(WmiMethod {
            name: method.to_string(),
            in_signature,
            argument,
        })
    }
}

impl WmiMethod {
    /// Name of the input parameter the driver declared.
    pub fn argument_name(&self) -> &str {
        &self.argument
    }

    /// Invokes `method` on `target`, returning the `argr` out-parameter or the
    /// reason the call did not produce a usable value.
    pub fn exec(&self, wmi: &Wmi, target: &str, arg: u32) -> Result<u32, WmiError> {
        let in_params = self
            .in_signature
            .spawn_instance()
            .ok_or(WmiError::new("IWbemClassObject::SpawnInstance", 0))?;
        if !in_params.put_u32(&self.argument, arg) {
            return Err(WmiError::new("IWbemClassObject::Put(argument)", 0));
        }

        let target_path = Bstr::new(target);
        let method_name = Bstr::new(&self.name);
        let mut out_params: *mut c_void = null_mut();
        let hr = unsafe {
            (wmi.services_vtbl().exec_method)(
                wmi.services.as_raw(),
                target_path.as_ptr(),
                method_name.as_ptr(),
                0,
                null_mut(),
                in_params.ptr.as_raw(),
                &mut out_params,
                null_mut(),
            )
        };
        if hr < 0 {
            return Err(WmiError::new("IWbemServices::ExecMethod", hr));
        }
        if out_params.is_null() {
            return Err(WmiError::new(
                "IWbemServices::ExecMethod (no out params)",
                0,
            ));
        }
        let out_params = WmiObject {
            ptr: unsafe { ComPtr::from_raw(out_params) },
        };
        let value = out_params
            .get_u32("argr")
            .ok_or(WmiError::new("out parameter 'argr'", hr))?;
        if value == 0xFFFF_FFFF {
            // The driver uses all-ones to say "this request failed".
            return Err(WmiError::new("method reported failure", -1));
        }
        Ok(value)
    }

    /// Like [`WmiMethod::exec`] but discards the reason for a failure.
    pub fn call(&self, wmi: &Wmi, target: &str, arg: u32) -> Option<u32> {
        self.exec(wmi, target, arg).ok()
    }
}

impl WmiObject {
    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires local WMI access; reads method metadata only"]
    fn method_parameter_names_exclude_system_properties() {
        let _com = Com::init(false).expect("initialize COM");
        let wmi = Wmi::connect("ROOT\\CIMV2").expect("connect to local WMI");
        let class = wmi.get_object("Win32_Process").expect("read process class");
        let signature = class.get_method("Create").expect("read method signature");
        let mut names = signature.get_names();
        names.sort();

        assert_eq!(
            names,
            [
                "CommandLine",
                "CurrentDirectory",
                "ProcessStartupInformation"
            ]
        );
        let method = wmi.method("Win32_Process", "Create").expect("read method");
        assert!(names.iter().any(|name| name == method.argument_name()));
    }
}
