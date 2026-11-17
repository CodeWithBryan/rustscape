use crate::{Result, run};
use std::path::Path;
pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    let output = run(&mut crate::login_oracle::invocation(
        root,
        args,
        "oracle/config/generate.py",
    )?)?;
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}
