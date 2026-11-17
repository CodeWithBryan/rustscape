use crate::{Result, run};
use rs09_server::auth::password::{PasswordCodec, Passwords};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    let output = run(&mut crate::login_oracle::invocation(
        root,
        args,
        "oracle/auth/generate.py",
    )?)?;
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}
pub fn interop(root: &Path, args: &[String]) -> Result<()> {
    let directory = root.join("runs/auth-interop");
    fs::create_dir_all(&directory)?;
    let file = directory.join("native-hashes.jsonl");
    let passwords = [
        String::new(),
        "password".into(),
        "éΩ😀".into(),
        "a".repeat(100),
    ];
    let codec = Passwords::default();
    let mut inputs = String::new();
    for password in &passwords {
        let stored = codec.hash(password)?;
        if !stored.starts_with("$2a$12$") {
            return Err("native default password work factor/version changed".into());
        }
        inputs.push_str(&serde_json::to_string(
            &json!({"kind":"verify","password":password,"stored":stored}),
        )?);
        inputs.push('\n');
    }
    fs::write(&file, inputs)?;
    let mut command = crate::login_oracle::invocation(root, args, "oracle/auth/generate.py")?;
    command.args(["--check", "--interop-file"]).arg(&file);
    let output = String::from_utf8(run(&mut command)?.stdout)?;
    let report: Value = serde_json::from_str(
        output
            .lines()
            .last()
            .ok_or("missing password interop report")?,
    )?;
    if report["nativeToJvm"].as_array().map(Vec::len) != Some(4)
        || report["jvmToNative"].as_array().map(Vec::len) != Some(4)
    {
        return Err("password probe counts differ".into());
    }
    for case in report["jvmToNative"].as_array().unwrap() {
        let password = case["password"]
            .as_str()
            .ok_or("missing oracle test password")?;
        let stored = case["stored"].as_str().ok_or("missing oracle hash")?;
        if !stored.starts_with("$2a$12$")
            || !codec.verify(password, stored)?
            || codec.verify(&format!("X{password}"), stored)?
        {
            return Err("native password check disagrees with original default-cost hash".into());
        }
    }
    let metadata = json!({"format":1,"upstream":crate::verify_reference(root)?,
        "scope":"Default cost-12 BCrypt with OS-generated salts in both runtimes, synthetic test credentials only. No SQL storage or playable-login claim.",
        "nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,
        "oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/auth/provenance.json"))?)?,"results":report});
    fs::write(
        directory.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&metadata)?),
    )?;
    println!(
        "Default-cost password interoperability passed: four native hashes checked by Java, four Java hashes checked by Rust."
    );
    Ok(())
}
