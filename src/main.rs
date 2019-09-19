use std::process::{self, Command, Stdio};
use std::path::PathBuf;
use std::fs;
use std::io;
use std::env;
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
use serde::Deserialize;

fn get_config<'a, 'de, T: Deserialize<'de>>(config_path: PathBuf, path_type: &'a str) -> T {
    let display_path = config_path.display();
    if !config_path.is_file() {
        eprintln!("No {} config file found at {}", path_type, display_path);
        process::exit(1);
    }
    let config_data = fs::read(&config_path);
    let config_data = match config_data {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Failed to read {} config file {}: {}", path_type, display_path, err);
            process::exit(1);
        }
    };
    let config_raw: Result<toml::Value, _> = toml::from_slice(&config_data);
    let config_raw = match config_raw {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Failed to parse {} config file {}: {}", path_type, display_path, err);
            process::exit(1);
        }
    };
    match config_raw.try_into() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Failed to parse {} config file {}: {}", path_type, display_path, err);
            process::exit(1);
        }
    }
}

/// Global settings
#[derive(Deserialize)]
struct GlobalConfig {
    github_token: String,
    web_root_path: PathBuf,
    web_root_url: String,
    fetch_interval: u64,
}

fn get_global_config() -> GlobalConfig {
    let mut config_path = dirs::config_dir().expect("Could not find user config directory");
    config_path.push("micro-ci-global.toml");
    get_config(config_path, "global")
}

/// Local settings
#[derive(Deserialize)]
struct LocalConfig {
    github_repo: String,
    command: String,
}

fn get_local_config() -> LocalConfig {
    let config_path = PathBuf::from(".micro-ci.toml");
    get_config(config_path, "local")
}

fn run_ci(gconfig: &GlobalConfig, lconfig: &LocalConfig) -> Result<(bool, Uuid), io::Error> {
    info!("Starting to run CI");
    let command = Command::new("bash")
        .arg("-c")
        .arg(format!("{} 2>&1", &lconfig.command))
        .output()?;

    let out_text = String::from_utf8(command.stdout).expect("invalid utf-8");
    let status = command.status;
    
    let run_id = Uuid::new_v4();
    let run_path: PathBuf = {
        let mut result = gconfig.web_root_path.clone();
        result.push(&lconfig.github_repo);
        fs::create_dir_all(&result)?;
        result.push(&format!("{}.txt", run_id));
        result
    };
    
    let status_code = status.code()
        .map(|x| format!("{}", x))
        .unwrap_or(String::from("???"));
    let status_description = format!("Build exited with code {}", status_code);
    info!("{}", &status_description);
    let final_result = format!("{}\n\n{}\n", out_text, status_description);
    fs::write(&run_path, &final_result)?;
    Ok((status.success(), run_id))
}

fn get_repo(gconfig: &GlobalConfig, lconfig: &LocalConfig)
    -> Repository<HttpsConnector<HttpConnector>> {
    let github = Github::new(
        format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        Credentials::Token(gconfig.github_token.clone()),
    );
    let pieces = lconfig.github_repo.split("/").collect::<Vec<_>>();
    let user = pieces[0];
    let repo = pieces[1];
    github.repo(user, repo)
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

fn push_ci_status(gconfig: &GlobalConfig, lconfig: &LocalConfig,
    state: State, description: String, url: Option<String>) -> Future<Status> {
    debug!("Pushing {:?}", (&state, &description, &url));
    let mut options = StatusOptions::builder(state);
    options.description(description);
    let url = url.unwrap_or_else(|| "https://example.com".to_string());
    options.target_url(url);
    let options = options.build();
    let repo = get_repo(gconfig, lconfig);
    let statuses = repo.statuses();
    Box::new(statuses.create(&get_current_commit(), &options)
        .map(|x| {
            debug!("Got status with state {:?}", &x.state);
            x
        }).map_err(|err| {
            error!("Got error when creating status: {:?}", &err);
            err
        }))
}

fn build_ci_status(gconfig: &GlobalConfig, lconfig: &LocalConfig,
    result: Result<(bool, Uuid), io::Error>) -> (State, String, Option<String>) {
    let url_root = format!("{}/{}", &gconfig.web_root_url, &lconfig.github_repo);
    match result {
        Ok((true, id)) => (
            State::Success,
            "Build completed successfully".to_string(),
            Some(format!("{}/{}.txt", url_root, id)),
        ),
        Ok((false, id)) => (
            State::Failure,
            "Build did not complete successfully".to_string(),
            Some(format!("{}/{}.txt", url_root, id)),
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
    let gconfig = get_global_config();
    let lconfig = get_local_config();
    Box::new(push_ci_status(&gconfig, &lconfig, state, description, None).and_then(move |_| {
        let result = run_ci(&gconfig, &lconfig);
        let (state, description, url) = build_ci_status(&gconfig, &lconfig, result);
        push_ci_status(&gconfig, &lconfig, state, description, url).map(|_| ())
    }).map_err(|err| eprintln!("Encountered error: {}", err)))
}

fn pull_if_needed() -> Result<bool, String> {
    debug!("Fetching");
    let fetch = Command::new("git")
        .arg("fetch")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
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
            debug!("Already up-to-date, not pulling");
            return Ok(false);
        }
        debug!("Pulling");
        let pull = Command::new("git")
            .arg("pull")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        pull.map(|_| true).map_err(|e| format!("Failed to git pull: {}", e))
    } else {
        Err("Could not git fetch".to_string())
    }
}

fn main() {
    env_logger::init();
    let gconfig = get_global_config();
    let _lconfig = get_local_config();
    let has_run_now = env::args().any(|x| x == "--run-now");
    let mut should_run_now = if has_run_now { Some(()) } else { None };

    let interval = Interval::new(Instant::now(), Duration::from_secs(gconfig.fetch_interval))
        .map_err(|err| eprintln!("Interval error: {}", err))
        .for_each(move |_| {
            let test = match should_run_now.take() {
                Some(()) => {
                    info!("Forced to run from command line");
                    true
                }
                None => match pull_if_needed() {
                    Ok(x) => x,
                    Err(e) => {
                        error!("Error while pulling: {}", e);
                        false
                    }
                }
            };
            if test {
                tokio::spawn(run_everything());
            }
            Ok(())
        });

    tokio::run(interval);
}
