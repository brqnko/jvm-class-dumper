use crate::injector::ClientTrait;

mod injector;
mod jvm;

pub mod bridge;
pub mod client;
pub mod console;
pub mod error;

fn process_attach() -> Result<(), error::Error> {
    let client = client::Client::new();
    injector::BAgentInjector::run(client)?;

    Ok(())
}

#[cfg(target_os = "windows")]
mod win {
    use windows::Win32::{Foundation::HINSTANCE, System::SystemServices::DLL_PROCESS_ATTACH};

    #[unsafe(no_mangle)]
    extern "system" fn DllMain(_: HINSTANCE, call_reason: u32, _: *mut ()) -> bool {
        if call_reason == DLL_PROCESS_ATTACH {
            std::thread::spawn(|| match super::process_attach() {
                Ok(_) => {}
                Err(e) => {
                    println!("error: {e:?}");
                }
            });
        }

        true
    }
}
