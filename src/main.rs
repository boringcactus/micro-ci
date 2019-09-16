use std::process::Command;
use std::path::PathBuf;
use std::fs;
use std::io;

use uuid::Uuid;

// TODO make this configurable
const LOG_DIR: &'static str = "/nfs/student/m/mhorn/public_html/ci/554-work";
const URL_ROOT: &'static str = "https://www.cs.unm.edu/~mhorn/ci/554-work/";

fn run_ci() -> Result<(bool, Uuid), io::Error> {
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
    Ok((status.success(), run_id))
}

fn main() {
    let runner_result = run_ci();
    match runner_result {
        Ok((good, id)) => {
            println!("{} {}{}.txt", good, URL_ROOT, id);
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}
