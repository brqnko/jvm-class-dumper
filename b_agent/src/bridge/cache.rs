use std::collections::HashMap;

pub struct ClassCache {
    cache: HashMap<String, jni::objects::JClass<'static>>,
}

impl ClassCache {
    pub fn new() -> Self {
        ClassCache {
            cache: HashMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        class_name: String,
        class: jni::objects::JClass<'static>,
    ) -> Result<(), crate::error::Error> {
        self.cache.insert(class_name, class);
        Ok(())
    }

    pub fn get(&self, class_name: &str) -> Option<&jni::objects::JClass<'static>> {
        self.cache.get(class_name)
    }
}
