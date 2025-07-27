use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

mod cache;

// save location of dumped classes
fn get_save_location() -> String {
    format!(
        "{}\\dumped\\",
        Path::new(env!("CARGO_MANIFEST_DIR")).display()
    )
}

// communicate with java side program
pub struct JavaBridge {
    cache: cache::ClassCache,
    jvm: jni::JavaVM,
    saved_classes: HashSet<String>,
}

impl JavaBridge {
    pub fn new(jvm: jni::JavaVM) -> Self {
        JavaBridge {
            cache: cache::ClassCache::new(),
            jvm,
            saved_classes: HashSet::new(),
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
    pub fn on_classfile_load_hook(
        &mut self,
        class_name: &str,
        class_data: Vec<u8>,
        client: &mut Box<dyn crate::injector::ClientTrait>,
    ) -> Result<Vec<String>, crate::error::Error> {
        match self.saved_classes.get(class_name) {
            Some(_) => {
                // class already saved, no need to retransform
                return Ok(vec![]);
            }
            None => {
                // save class
                let save_path =
                    PathBuf::from(get_save_location()).join(format!("{}.class", (class_name.replace('.', "\\"))));
                std::fs::create_dir_all(save_path.parent().unwrap())?;
                std::fs::write(save_path, &class_data)?;
                println!("saved class: {class_name}");

                self.saved_classes.insert(class_name.to_string());
            }
        }
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
                "([B)[Ljava/lang/String;",
            )?;

            let res = env
                .call_static_method_unchecked(
                    retransformer,
                    method_id,
                    jni::signature::ReturnType::Array,
                    &[jni::sys::jvalue {
                        l: env.byte_array_from_slice(&class_data)?.as_raw(),
                    }],
                )?
                .l()?;

            let byte_array = jni::objects::JObjectArray::from_raw(res.as_raw());
            let classes = (0..env.get_array_length(&byte_array)?)
                .map(|i| {
                    let class_name = env.get_object_array_element(&byte_array, i)?;
                    let binding = jni::objects::JString::from(class_name);
                    let class_name_str = env.get_string(&binding)?;
                    Ok(class_name_str.to_string_lossy().into_owned())
                })
                .collect::<Result<Vec<String>, crate::error::Error>>()?;

            Ok(classes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_location() {
        // check the save location
        let path = get_save_location();
        println!("save location: {}", path);
    }
}
