#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("value not of type '{0}'")]
    XValueNotOfType(&'static str),

    #[error(transparent)]
    JNI(#[from] jni::errors::Error),

    #[error("jvmti error '{0}'")]
    JVMTI(String),

    #[error(transparent)]
    IO(#[from] std::io::Error),
}
