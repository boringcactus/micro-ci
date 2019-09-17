use std::process::Command;
use std::path::PathBuf;
use std::fs;
use std::io;
use std::time::{Instant, Duration};

#[macro_use] extern crate log;

use uuid::Uuid;
use hubcaps::{Credentials, Github, Future};
use hubcaps::repositories::Repository;
use hubcaps::statuses::{State, StatusOptions, Status};
use hyper::rt::{Future as StdFuture};
use hyper::client::HttpConnector;
use hyper_tls::HttpsConnector;
use tokio::prelude::*;
use tokio::timer::Interval;

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

fn get_repo() -> Repository<HttpsConnector<HttpConnector>> {
    let github = Github::new(
        format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        // TODO fetch this at runtime, not build time
        Credentials::Token(env!("GITHUB_TOKEN").to_string()),
    );
    // TODO don't hard code to just be my repo
    github.repo("boringcactus", "cs554-work")
}

fn get_current_commit() -> String {
    let out = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .expect("Failed to get git commit")
        .stdout;
    String::from_utf8(out).expect("invalid utf-8").trim().to_string()
    // TODO validate output
}

fn push_ci_status(state: State, description: String, url: Option<String>) -> Future<Status> {
    debug!("Pushing {:?}", (&state, &description, &url));
    let mut options = StatusOptions::builder(state);
    options.description(description);
    let url = url.unwrap_or_else(|| "https://example.com".to_string());
    options.target_url(url);
    let options = options.build();
    let repo = get_repo();
    let statuses = repo.statuses();
    Box::new(statuses.create(&get_current_commit(), &options)
        .map(|x| {
            debug!("Got status: {:?}", &x);
            x
        }).map_err(|err| {
            error!("Got error when creating status: {:?}", &err);
            err
        }))
}

fn build_ci_status(result: Result<(bool, Uuid), io::Error>) -> (State, String, Option<String>) {
    match result {
        Ok((true, id)) => (
            State::Success,
            "Build completed successfully".to_string(),
            Some(format!("{}{}.txt", URL_ROOT, id)),
        ),
        Ok((false, id)) => (
            State::Failure,
            "Build did not complete successfully".to_string(),
            Some(format!("{}{}.txt", URL_ROOT, id)),
        ),
        Err(e) => (
            State::Error,
            format!("micro-ci error: {}", e),
            None
        )
    }
}

fn run_everything() -> Box<dyn StdFuture<Item = (), Error = ()> + Send> {
    let state = State::Pending;
    let description = "Running build".to_string();
    Box::new(push_ci_status(state, description, None).and_then(|_| {
        let result = run_ci();
        let (state, description, url) = build_ci_status(result);
        push_ci_status(state, description, url).map(|_| ())
    }).map_err(|err| eprintln!("Encountered error: {}", err)))
}

fn pull_if_needed() -> Result<bool, String> {
    let fetch = Command::new("git")
        .arg("fetch")
        .status()
        .map_err(|e| format!("failed to git fetch: {}", e))?;
    if fetch.success() {
        let local = get_current_commit();
        let remote = {
            let out = Command::new("git")
                .arg("rev-parse")
                .arg("@{u}")
                .output()
                .expect("Failed to check for new git commit")
                .stdout;
            String::from_utf8(out).expect("invalid utf-8").trim().to_string()
        };
        if local == remote {
            return Ok(false);
        }
        let pull = Command::new("git").arg("pull").status();
        pull.map(|_| true).map_err(|e| format!("Failed to git pull: {}", e))
    } else {
        Err("Could not git fetch".to_string())
    }
}

fn main() {
    env_logger::init();

    let interval = Interval::new(Instant::now(), Duration::from_secs(60))
        .map_err(|err| eprintln!("Interval error: {}", err))
        .for_each(|_| {
            if let Ok(true) = pull_if_needed() {
                println!("Change caught, running tests");
                tokio::spawn(run_everything());
            }
            Ok(())
        });

    tokio::run(interval);
}
