use crate::{Result, run};
use std::path::Path;
pub fn generate(root: &Path, args: &[String], kind: &str) -> Result<()> {
    let output = run(&mut crate::login_oracle::invocation(
        root,
        args,
        &format!("oracle/{kind}/generate.py"),
    )?)?;
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}
