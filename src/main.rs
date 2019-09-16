use std::process::Command;
use std::path::PathBuf;
use std::fs;
use std::io;

use uuid::Uuid;

// TODO make this configurable
const LOG_DIR: &'static str = "/nfs/student/m/mhorn/public_html/ci/554-work";

fn run_ci() -> Result<(bool, PathBuf), io::Error> {
    // TODO make this configurable
    let command = Command::new("bash")
        .arg("-c")
        .arg("cargo test 2>&1")
        .output()?;

    let out_text = String::from_utf8(command.stdout).expect("invalid utf-8");
    let status = command.status;
    
    let run_id = Uuid::new_v4();
    let run_path: PathBuf = [LOG_DIR, &format!("{}.txt", run_id)].iter().collect();
    
    let status_code = status.code()
        .map(|x| format!("{}", x))
        .unwrap_or(String::from("???"));
    let final_result = format!("{}\n\nBuild exited with code {}\n", out_text, status_code);
    fs::write(&run_path, &final_result)?;
    Ok((status.success(), run_path))
}

fn main() {
    let result = run_ci();
    println!("{:?}", &result);
}
