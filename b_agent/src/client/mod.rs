use std::{collections::HashMap, fs::File, io::Read as _, path::Path, sync::LazyLock};

use zip::ZipArchive;

use crate::injector::ClientTrait;

pub struct Client {}

// must be the same as in java side shadowJar file path
fn get_client_classes_path() -> String {
    format!(
        "{}\\b_client\\build\\libs\\b_client-1.0-SNAPSHOT-all.jar",
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .display()
    )
}

// lazy load client classes from jar file
static CLIENT_CLASSES: LazyLock<HashMap<String, Vec<u8>>> = LazyLock::new(|| {
    let mut f = File::open(get_client_classes_path()).unwrap();
    let mut buf = vec![];
    f.read_to_end(&mut buf).unwrap();
    let cursor = std::io::Cursor::new(buf);

    // unzip the jar file
    let mut archive = ZipArchive::new(cursor).unwrap();

    let mut map: HashMap<String, Vec<u8>> = HashMap::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let file_name = file.name().to_string();

        if !file_name.ends_with(".class") {
            continue;
        }

        let mut class_data = Vec::new();
        file.read_to_end(&mut class_data).unwrap();

        map.insert(file_name, class_data);
    }

    map
});

impl ClientTrait for Client {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    // return client classes, which are used by hook
    // we have to divide classes because retransform classes are called by Rust side
    fn client_classes(
        &self,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, crate::error::Error> {
        Ok(CLIENT_CLASSES
            .iter()
            .map(|(name, data)| (name.clone(), data.clone()))
            .collect())
    }

    // class names to retransform
    // bew is net.minecraft.client.entity.EntityPlayerSP
    fn class_names_to_retransform(&self) -> Result<Vec<String>, crate::error::Error> {
        Ok(vec!["bew".to_string()])
    }

    // return classes to retransform
    fn retransform_classes(
        &self,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, crate::error::Error> {
        Ok(CLIENT_CLASSES
            .iter()
            .map(|(name, data)| (name.clone(), data.clone()))
            .collect())
    }

    // the full class name of the retransformer class
    fn retransformer_class_name(&self) -> &str {
        "io.github.brqnko.retransformer.Retransformer"
    }

    // the name of the retransform method in the retransformer class
    fn retransform_method_name(&self) -> &str {
        "retransform"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_class_path() {
        // check the client classes path
        let path = get_client_classes_path();
        println!("client classes path: {}", path);
    }
}

