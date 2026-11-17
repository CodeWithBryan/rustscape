//! Explicit live launch configuration; credentials are neither logged nor persisted.
pub struct Launch {
    pub endpoint: String,
    /// Empty when the launch names no account: the title screen asks.
    pub username: Vec<u8>,
    pub password: Vec<u8>,
}
fn latin1(s: &str) -> Result<Vec<u8>, String> {
    s.chars()
        .map(|c| {
            u8::try_from(c as u32).map_err(|_| "Login requires original Latin-1 characters".into())
        })
        .collect()
}
#[cfg(not(target_arch = "wasm32"))]
pub fn read() -> Result<Option<Launch>, String> {
    let args: Vec<_> = std::env::args().collect();
    let option = |key: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let Some(endpoint) = option("--connect") else {
        return Ok(None);
    };
    // Without `--username` the player logs in at the title screen.
    let Some(username) = option("--username") else {
        return Ok(Some(Launch {
            endpoint,
            username: Vec::new(),
            password: Vec::new(),
        }));
    };
    let password = std::env::var("RS09_CLIENT_PASSWORD")
        .map_err(|_| "RS09_CLIENT_PASSWORD is required with --username")?;
    Ok(Some(Launch {
        endpoint,
        username: latin1(&username)?,
        password: latin1(&password)?,
    }))
}
#[cfg(target_arch = "wasm32")]
pub fn read() -> Result<Option<Launch>, String> {
    unsafe extern "C" {
        fn rs09_launch_value(index: i32, ptr: *mut u8, capacity: usize) -> i32;
    }
    let mut values = vec![];
    for index in 0..3 {
        let mut bytes = vec![0; 2048];
        let n = unsafe { rs09_launch_value(index, bytes.as_mut_ptr(), bytes.len()) };
        if n < 0 {
            return Err("Invalid browser launch configuration".into());
        }
        bytes.truncate(n as usize);
        values
            .push(String::from_utf8(bytes).map_err(|_| "Invalid browser configuration encoding")?);
    }
    if values[0].is_empty() {
        return Ok(None);
    }
    Ok(Some(Launch {
        endpoint: values.remove(0),
        username: latin1(&values.remove(0))?,
        password: latin1(&values.remove(0))?,
    }))
}
pub fn publish(state: &serde_json::Value) {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe extern "C" {
            fn rs09_live_state(ptr: *const u8, len: usize);
        }
        let bytes = state.to_string();
        unsafe {
            rs09_live_state(bytes.as_ptr(), bytes.len());
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = state;
    }
}
/// Opt-in app-submitted audio proof; normal play does not retain PCM samples.
pub fn audio_review() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe extern "C" {
            fn rs09_pcm_record() -> i32;
        }
        unsafe { rs09_pcm_record() != 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}
/// Explicit offline packet fixture, independent of live login credentials.
pub fn npc_replay_path() -> Result<Option<String>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<_> = std::env::args().collect();
        Ok(args
            .iter()
            .position(|a| a == "--npc-replay")
            .and_then(|i| args.get(i + 1))
            .cloned())
    }
    #[cfg(target_arch = "wasm32")]
    {
        unsafe extern "C" {
            fn rs09_launch_value(index: i32, ptr: *mut u8, capacity: usize) -> i32;
        }
        let mut bytes = vec![0; 2048];
        let n = unsafe { rs09_launch_value(3, bytes.as_mut_ptr(), bytes.len()) };
        if n < 0 {
            return Err("Invalid NPC preview path".into());
        }
        bytes.truncate(n as usize);
        let value = String::from_utf8(bytes).map_err(|_| "Invalid NPC preview path")?;
        Ok((!value.is_empty()).then_some(value))
    }
}

/// Explicit offline equipped appearance; never used to override a live player.
pub fn appearance_preview_path() -> Result<Option<String>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<_> = std::env::args().collect();
        Ok(args
            .iter()
            .position(|a| a == "--appearance-preview")
            .and_then(|i| args.get(i + 1))
            .cloned())
    }
    #[cfg(target_arch = "wasm32")]
    {
        unsafe extern "C" {
            fn rs09_launch_value(index: i32, ptr: *mut u8, capacity: usize) -> i32;
        }
        let mut bytes = vec![0; 2048];
        let n = unsafe { rs09_launch_value(4, bytes.as_mut_ptr(), bytes.len()) };
        if n < 0 {
            return Err("Invalid appearance preview path".into());
        }
        bytes.truncate(n as usize);
        let value = String::from_utf8(bytes).map_err(|_| "Invalid appearance preview path")?;
        Ok((!value.is_empty()).then_some(value))
    }
}
