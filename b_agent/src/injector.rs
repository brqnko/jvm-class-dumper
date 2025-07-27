use jvmti::environment::jvmti::JVMTI;
use jvmti::{
    environment::{Environment, jni::JNIEnvironment, jvmti::JVMTIEnvironment},
    native::{
        JNIEnvPtr, JVMTIEnvPtr, JavaClass, JavaObject, RawString,
        jvmti_native::{jint, jvmtiEventCallbacks},
    },
};
use libc::{c_char, c_uchar};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::{ffi::CStr, ptr::copy_nonoverlapping, sync::Mutex};

use crate::console::{alloc_console, free_console};

const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

pub trait ClientTrait: Send + Sync + 'static {
    fn new() -> Self
    where
        Self: Sized;

    fn on_classfile_load_hook(&self) -> Result<HashMap<String, Vec<u8>>, crate::error::Error>;

    fn retransformer_class_name(&self) -> &str;

    fn retransform_method_name(&self) -> &str;
}

// global bridge instance
static BRIDGE: Mutex<Option<crate::bridge::JavaBridge>> = Mutex::new(None);

// global client instance
static CLIENT: Mutex<Option<Box<dyn ClientTrait>>> = Mutex::new(None);

static CLASSES_TO_LOAD: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

static LOADED_CLASSES: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg(target_os = "windows")]
unsafe fn exit_dll() {
    use winapi::um::libloaderapi::{FreeLibraryAndExitThread, GetModuleHandleA};
    use winapi::um::wincon::FreeConsole;
    use windows::core::s;

    unsafe {
        FreeConsole();
        // must be same as the dll file name
        let module = GetModuleHandleA(s!("b_agent.dll").as_ptr() as *const i8);
        FreeLibraryAndExitThread(module, 0);
    }
}

pub struct BAgentInjector {
    jvm: jni::JavaVM,
    jvmti: jvmti::environment::jvmti::JVMTIEnvironment,
    jvmti_raw: jvmti::native::JVMTIEnvPtr,
}

impl Drop for BAgentInjector {
    fn drop(&mut self) {
        unsafe { self.jvm.detach_current_thread() };
        BRIDGE.lock().unwrap().take();
        CLIENT.lock().unwrap().take();

        println!("You may close this window now.");
        unsafe {
            // free console
            free_console();

            exit_dll();
        }
    }
}

impl BAgentInjector {
    pub fn run(client: impl ClientTrait) -> Result<Self, crate::error::Error> {
        unsafe { alloc_console()? }

        let jvm = crate::jvm::get_jvm()?;
        // NOTE: _env is not used, but it is required to keep the thread attached to the JVM
        let mut _env = jvm.attach_current_thread()?;
        let jvm_ptr = jvm.get_java_vm_pointer();

        unsafe {
            let jvmti_raw = std::alloc::alloc(std::alloc::Layout::new::<
                jvmti::native::jvmti_native::jvmtiEnv,
            >()) as *mut *mut std::ffi::c_void;
            let Some(get_env) = (**jvm_ptr).GetEnv else {
                return Err(crate::error::Error::XValueNotOfType("get_env"));
            };
            if get_env(
                jvm_ptr,
                jvmti_raw,
                jvmti::native::jvmti_native::JVMTI_VERSION_1_2 as i32,
            ) != jvmti::native::jvmti_native::JVMTI_ERROR_NONE as i32
            {
                return Err(crate::error::Error::XValueNotOfType("jvm env"));
            }

            let mut jvmti = jvmti::environment::jvmti::JVMTIEnvironment::new(
                *jvmti_raw as jvmti::native::JVMTIEnvPtr,
            );

            // set capabilities to retransform classes
            let mut capabilities = jvmti.get_capabilities();
            capabilities.can_redefine_classes = true;
            capabilities.can_redefine_any_class = true;
            capabilities.can_retransform_classes = true;
            capabilities.can_retransform_any_class = true;
            if let Result::Err(e) = jvmti.add_capabilities(&capabilities) {
                return Err(crate::error::Error::JVMTI(jvmti::error::translate_error(
                    &e,
                )));
            }

            // set class file load hook
            let native_callbacks = jvmti::native::jvmti_native::jvmtiEventCallbacks {
                ClassFileLoadHook: Some(local_cb_class_file_load_hook),
                ..Default::default()
            };
            let jvmti_env_ptr = *jvmti_raw as jvmti::native::JVMTIEnvPtr;
            let Some(set_event_callbacks) = (**jvmti_env_ptr).SetEventCallbacks else {
                return Err(crate::error::Error::XValueNotOfType("set event callbacks"));
            };
            set_event_callbacks(
                jvmti_env_ptr,
                &native_callbacks,
                size_of::<jvmtiEventCallbacks>() as i32,
            );

            BRIDGE
                .lock()
                .unwrap()
                .replace(crate::bridge::JavaBridge::new(jni::JavaVM::from_raw(
                    jvm_ptr,
                )?));

            CLIENT.lock().unwrap().replace(Box::new(client));

            let mut me = Self {
                jvm: jni::JavaVM::from_raw(jvm_ptr)?,
                jvmti,
                jvmti_raw: *jvmti_raw as jvmti::native::JVMTIEnvPtr,
            };
            me.run_internal()?;

            Ok(me)
        }
    }

    fn run_internal(&mut self) -> Result<(), crate::error::Error> {
        load_client_classes(
            &mut self.jvm.get_env()?,
            self.jvmti_raw,
            &crate::jvm::get_url_class(&mut self.jvm.get_env()?)?,
            CLIENT.lock().unwrap().as_mut().unwrap(),
            BRIDGE.lock().unwrap().as_mut().unwrap(),
        )?;

        self.jvmti
            .set_event_notification_mode(jvmti::event::VMEvent::ClassFileLoadHook, true);

        println!("Waiting for classes to be loaded...");

        std::thread::sleep(std::time::Duration::from_secs(10));
        while !CLASSES_TO_LOAD.lock().unwrap().is_empty() {
            let taken = CLASSES_TO_LOAD.lock().unwrap().drain().collect::<Vec<_>>();
            let classes = load_classes_to_retransform(&mut self.jvm.get_env()?, taken.clone())?;
            for class in classes {
                unsafe {
                    let Some(retransform_classes) = (**self.jvmti_raw).RetransformClasses else {
                        return Err(crate::error::Error::XValueNotOfType("retransform classes"));
                    };

                    retransform_classes(
                        self.jvmti_raw,
                        1,
                        &(class.as_raw() as jvmti::native::JavaClass),
                    );
                }
            }
        }

        self.jvmti
            .set_event_notification_mode(jvmti::event::VMEvent::ClassFileLoadHook, false);

        Ok(())
    }
}

fn load_client_classes<'a>(
    env: &mut jni::JNIEnv<'a>,
    jvmti_raw: jvmti::native::JVMTIEnvPtr,
    retransform_context_class: &jni::objects::JClass<'a>,
    client: &mut Box<dyn ClientTrait>,
    bridge: &mut crate::bridge::JavaBridge,
) -> Result<(), crate::error::Error> {
    fn load_classes<'a>(
        env: &mut jni::JNIEnv<'a>,
        jvmti_raw: jvmti::native::JVMTIEnvPtr,
        context_class: &jni::objects::JClass<'a>,
        mut classes: HashMap<String, Vec<u8>>,
        bridge: &mut crate::bridge::JavaBridge,
    ) -> Result<(), crate::error::Error> {
        let mut class_loader =
            unsafe { std::alloc::alloc(std::alloc::Layout::new::<jni::sys::jobject>()) }
                as *mut jvmti::native::jvmti_native::Struct__jobject;
        unsafe {
            let Some(get_class_loader) = (**jvmti_raw).GetClassLoader else {
                return Err(crate::error::Error::XValueNotOfType("get class loader"));
            };
            get_class_loader(
                jvmti_raw,
                context_class.as_raw() as *mut jvmti::native::jvmti_native::Struct__jobject,
                &mut class_loader,
            );
        }

        while !classes.is_empty() {
            std::thread::sleep(CHECK_INTERVAL);
            let mut loaded_class_names = Vec::new();

            for (name, bytes) in classes.iter() {
                unsafe {
                    let Some(define_class) = (**env.get_raw()).DefineClass else {
                        continue;
                    };

                    let class_copy = define_class(
                        env.get_raw(),
                        std::ptr::null(),
                        class_loader as *mut jni::sys::_jobject,
                        bytes.as_ptr() as *const jni::sys::jbyte,
                        bytes.len() as i32,
                    );
                    if class_copy.is_null() {
                        continue;
                    }

                    let class_name = name.replace("/", ".").replace(".class", "");
                    loaded_class_names.push(class_name.clone());
                    bridge.insert_cache(
                        class_name.clone(),
                        jni::objects::JClass::from_raw(class_copy),
                    )?;
                }
            }

            for name in loaded_class_names.iter() {
                classes.remove(name);
            }

            if loaded_class_names.is_empty() {
                println!("failed to load classes: {:?}", classes.keys());
                println!("skipping retransforming");
                break;
            } else {
                println!("successfully loaded classes: {loaded_class_names:#?}");
            }
        }

        Ok(())
    }

    load_classes(
        env,
        jvmti_raw,
        retransform_context_class,
        client.on_classfile_load_hook()?,
        bridge,
    )?;

    Ok(())
}

fn class_file_load_hook(class_name: &str, class_data: Vec<u8>) -> Option<Vec<u8>> {
    LOADED_CLASSES
        .lock()
        .unwrap()
        .insert(class_name.to_string());

    match BRIDGE
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .on_classfile_load_hook(
            class_name,
            class_data,
            CLIENT.lock().unwrap().as_mut().unwrap(),
        ) {
        Ok(dependencies) => {
            let mut to_loade_lock = CLASSES_TO_LOAD.lock().unwrap();
            let loaded_lock = LOADED_CLASSES.lock().unwrap();
            for name in dependencies {
                if loaded_lock.contains(&name) {
                    continue;
                }

                to_loade_lock.insert(name);
            }
        }
        Err(e) => {
            println!("Error in class_file_load_hook: {e}");
        }
    }

    None
}

#[allow(warnings)]
unsafe extern "C" fn local_cb_class_file_load_hook(
    jvmti_env: JVMTIEnvPtr,
    jni_env: JNIEnvPtr,
    _class_being_redefined: JavaClass,
    _loader: JavaObject,
    name: *const c_char,
    _protection_domain: JavaObject,
    class_data_len: jint,
    class_data: *const c_uchar,
    new_class_data_len: *mut jint,
    new_class_data: *mut *mut c_uchar,
) {
    let mut raw_data: Vec<u8> = Vec::with_capacity(class_data_len as usize);
    let data_ptr = raw_data.as_mut_ptr();

    copy_nonoverlapping(class_data, data_ptr, class_data_len as usize);
    raw_data.set_len(class_data_len as usize);
    if let Some(transformed) = class_file_load_hook(&stringify(name), raw_data) {
        let env = Environment::new(
            JVMTIEnvironment::new(jvmti_env),
            JNIEnvironment::new(jni_env),
        );

        match env.allocate(transformed.len()) {
            Ok(allocation) => {
                copy_nonoverlapping(transformed.as_ptr(), allocation.ptr, allocation.len);
                *new_class_data_len = allocation.len as i32;
                *new_class_data = allocation.ptr;
            }
            Err(_) => {
                println!("failed to allocate memory");
            }
        }
    };
}

fn load_classes_to_retransform<'a>(
    env: &mut jni::JNIEnv<'a>,
    class_names_to_retransform: Vec<String>,
) -> Result<Vec<jni::objects::JClass<'a>>, crate::error::Error> {
    let classes = class_names_to_retransform
        .into_iter()
        .map(|class_name| {
            loop {
                let a = crate::jvm::find_class(env, &class_name);
                match a {
                    Ok(class) => {
                        break unsafe { jni::objects::JClass::from_raw(class.as_raw()) };
                    }
                    _ => std::thread::sleep(CHECK_INTERVAL),
                }
            }
        })
        .collect::<Vec<_>>();

    Ok(classes)
}

pub fn stringify(input: RawString) -> String {
    unsafe {
        if !input.is_null() {
            match CStr::from_ptr(input).to_str() {
                Ok(string) => string.to_string(),
                Err(_) => "(UTF8-ERROR)".to_string(),
            }
        } else {
            "(NULL)".to_string()
        }
    }
}
