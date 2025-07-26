mod cache;

// communicate with java side program
pub struct JavaBridge {
    cache: cache::ClassCache,
    jvm: jni::JavaVM,
}

impl JavaBridge {
    pub fn new(jvm: jni::JavaVM) -> Self {
        JavaBridge {
            cache: cache::ClassCache::new(),
            jvm,
        }
    }

    pub fn insert_cache(
        &mut self,
        class_name: String,
        class: jni::objects::JClass<'static>,
    ) -> Result<(), crate::error::Error> {
        self.cache.insert(class_name, class)
    }

    // retransform class using the retransformer class from java side program
    pub fn retransform_class(
        &self,
        class_name: &str,
        class_data: Vec<u8>,
        client: &mut Box<dyn crate::injector::ClientTrait>,
    ) -> Result<Vec<u8>, crate::error::Error> {
        let mut env = self.jvm.get_env()?;
        let Some(retransformer) = self.cache.get(client.retransformer_class_name()) else {
            return Err(crate::error::Error::XValueNotOfType(
                "retransformer class not found",
            ));
        };

        unsafe {
            let method_id = env.get_static_method_id(
                retransformer,
                client.retransform_method_name(),
                "([BLjava/lang/String;)[B",
            )?;

            let res = env
                .call_static_method_unchecked(
                    retransformer,
                    method_id,
                    jni::signature::ReturnType::Array,
                    &[
                        jni::sys::jvalue {
                            l: env.byte_array_from_slice(&class_data)?.as_raw(),
                        },
                        jni::sys::jvalue {
                            l: env.new_string(class_name)?.as_raw(),
                        },
                    ],
                )?
                .l()?;

            let byte_array = jni::objects::JByteArray::from_raw(res.as_raw());
            let mut buf = vec![0; env.get_array_length(&byte_array)? as usize];
            env.get_byte_array_region(&byte_array, 0, &mut buf)?;

            Ok(buf.iter().map(|&x| x as u8).collect())
        }
    }
}
