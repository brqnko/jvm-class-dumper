use std::ffi::{c_int, c_void};

type GetCreatedJavaVMs = extern "system" fn(*mut *mut c_void, c_int, *mut c_int) -> c_int;

#[cfg(target_os = "windows")]
fn get_jni_get_created_jvms() -> Option<GetCreatedJavaVMs> {
    use winapi::um::libloaderapi::{GetModuleHandleA, GetProcAddress};
    use windows::core::s;
    let jvm_module = unsafe { GetModuleHandleA(s!("jvm.dll").as_ptr() as *const i8) };
    if jvm_module.is_null() {
        return None;
    }

    let jvm_proc_address = unsafe {
        GetProcAddress(
            jvm_module,
            s!("JNI_GetCreatedJavaVMs").as_ptr() as *const i8,
        )
    };
    if jvm_proc_address.is_null() {
        return None;
    }

    let get_created_jvm = unsafe { std::mem::transmute(jvm_proc_address) };

    Some(get_created_jvm)
}

pub fn get_jvm() -> Result<jni::JavaVM, crate::error::Error> {
    let mut jvm_ciunt = 0;

    let mut jvm_raw = Vec::<*mut std::ffi::c_void>::with_capacity(1);

    let Some(get_jvms) = get_jni_get_created_jvms() else {
        return Err(crate::error::Error::XValueNotOfType(
            "get_jni_get_created_jvms",
        ));
    };
    if get_jvms(jvm_raw.as_mut_ptr(), 1, &mut jvm_ciunt)
        != jvmti::native::jvmti_native::JVMTI_ERROR_NONE as i32
    {
        return Err(crate::error::Error::XValueNotOfType("active jvm"));
    }

    unsafe {
        jvm_raw.set_len(jvm_ciunt as usize);
    }
    let Some(jvm) = jvm_raw.first() else {
        return Err(crate::error::Error::XValueNotOfType("first jvm"));
    };

    let jvm = unsafe { jni::JavaVM::from_raw(*jvm as *mut jni::sys::JavaVM) }?;

    Ok(jvm)
}

pub fn find_class<'a>(
    env: &mut jni::JNIEnv<'a>,
    class_name: &str,
) -> Result<jni::objects::JObject<'a>, crate::error::Error> {
    if let Ok(class) = env.find_class(class_name)
        && !class.is_null()
    {
        return Ok(unsafe { jni::objects::JObject::from_raw(class.as_raw()) });
    }

    let stack_traces_map = env
        .call_static_method(
            "java/lang/Thread",
            "getAllStackTraces",
            "()Ljava/util/Map;",
            &[],
        )?
        .l()?;
    let threads_set = env
        .call_method(&stack_traces_map, "keySet", "()Ljava/util/Set;", &[])?
        .l()?;
    let threads = env
        .call_method(&threads_set, "toArray", "()[Ljava/lang/Object;", &[])?
        .l()?;
    let threads_array = jni::objects::JObjectArray::from(threads);
    let threads_amount: i32 = env.get_array_length(&threads_array)?;
    let klass = env.find_class("java/lang/ClassLoader")?;
    let get_class =
        env.get_method_id(&klass, "findClass", "(Ljava/lang/String;)Ljava/lang/Class;")?;

    for i in 0..threads_amount {
        let thread = env.get_object_array_element(&threads_array, i);
        if let Err(_) = thread {
            continue;
        }
        let thread = thread?;
        let class_loader = env
            .call_method(
                &thread,
                "getContextClassLoader",
                "()Ljava/lang/ClassLoader;",
                &[],
            )?
            .l()?;
        if !class_loader.is_null() {
            let class = unsafe {
                env.call_method_unchecked(
                    &class_loader,
                    get_class,
                    jni::signature::ReturnType::Object,
                    &[jni::sys::jvalue {
                        l: env.new_string(class_name)?.as_raw(),
                    }],
                )
            };
            if let Err(_) = class {
                continue;
            }
            let class = class?.l();
            if let Err(_) = class {
                continue;
            }
            let class = class?;
            if !class.is_null() {
                return Ok(class);
            }
        }
    }

    let attempt = env.find_class(class_name);
    if let Ok(class) = attempt {
        if !class.is_null() {
            return Ok(unsafe { jni::objects::JObject::from_raw(class.as_raw()) });
        }
    }

    Err(crate::error::Error::XValueNotOfType("find_class"))
}

pub fn get_url_class<'a>(
    env: &mut jni::JNIEnv,
) -> Result<jni::objects::JClass<'a>, crate::error::Error> {
    let url_class = env.find_class("java/net/URL")?;
    if url_class.is_null() {
        return Err(crate::error::Error::XValueNotOfType("java/net/URL"));
    }

    Ok(unsafe { jni::objects::JClass::from_raw(url_class.as_raw()) })
}
