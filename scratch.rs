use std::process::Command;

fn main() {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("npm --version");
    
    // mimic exactly ShellTool
    cmd.env_clear();
    let safe_keys = ["PATH", "HOME", "TERM", "LANG", "LC_ALL", "LC_CTYPE", "USER", "SHELL", "TMPDIR"];
    for key in safe_keys {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }
    
    let output = cmd.output().unwrap();
    println!("status: {:?}", output.status);
    println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
}
