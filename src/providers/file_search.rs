//! File search subprocess provider
//!
//! This module provides file search and grep functionality by executing
//! system commands (plocate, find, rg, grep) as subprocesses.
//! Results are delivered asynchronously via channels.

use std::path::Path;

use gtk4::glib;
use gtk4::prelude::ListModelExt;

use crate::actions::which;
use crate::core::global_state::get_home_dir;
use crate::model::items::CommandItem;
use crate::model::list_model::AppListModel;
use crate::providers::{SubprocessRunner, spawn_subprocess};

/// Run a subprocess command and collect its output in a background thread
///
/// The command output is sent back to the main thread via a channel,
/// then processed by a `SubprocessRunner` to update the UI.
pub fn run_subprocess(model: &AppListModel, cmd: std::process::Command) {
    let generation = model.state.task_gen();
    let max_results = model.config.max_results.get();
    let model_clone = model.clone();

    let (tx, rx) = std::sync::mpsc::channel::<Vec<String>>();

    spawn_subprocess(move || cmd, max_results, tx);

    let processor = |model: &AppListModel, _gen: u64, lines: Vec<String>| {
        model.store.remove_all();
        for line in lines {
            model.store.append(&CommandItem::new(line));
        }
        if model.store.n_items() > 0 && model.selection.selected() == gtk4::INVALID_LIST_POSITION {
            model.selection.set_selected(0);
        }
    };
    let runner = SubprocessRunner::new(rx, model_clone, generation, processor);
    glib::idle_add_local_once(move || runner.poll());
}

/// Execute a file search command without using shell
pub fn run_file_search(model: &AppListModel, argument: &str) {
    let command = if which("plocate").is_some() {
        let mut cmd = std::process::Command::new("plocate");
        cmd.arg("-i")
            .arg("--")
            .arg(argument)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        cmd
    } else {
        let home = get_home_dir();
        let mut cmd = std::process::Command::new("find");
        cmd.arg(home)
            .arg("-type")
            .arg("f")
            .arg("-iname")
            .arg(format!("*{argument}*"))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        cmd
    };

    run_subprocess(model, command);
}

/// Execute a file grep command without using shell
pub fn run_file_grep(model: &AppListModel, argument: &str) {
    let command = if which("rg").is_some() {
        let home = get_home_dir();
        let mut cmd = std::process::Command::new("rg");
        cmd.arg("--with-filename")
            .arg("--line-number")
            .arg("--no-heading")
            .arg("-i")
            .arg(argument)
            .arg(home)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        cmd
    } else {
        let home = get_home_dir();
        let mut cmd = std::process::Command::new("grep");
        cmd.arg("-r")
            .arg("-i")
            .arg("-n")
            .arg("-I")
            .arg("-H")
            .arg("--")
            .arg(argument)
            .arg(home)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        cmd
    };

    run_subprocess(model, command);
}

/// Run `find` command to search for files in Obsidian vault
pub fn run_find_in_vault(model: &AppListModel, vault_path: &Path, pattern: &str) {
    let mut cmd = std::process::Command::new("find");
    cmd.arg(vault_path)
        .arg("-type")
        .arg("f")
        .arg("-iname")
        .arg(format!("*{pattern}*"));
    run_subprocess(model, cmd);
}

/// Run `rg` (ripgrep with grep fallback) command to search file contents in Obsidian vault
pub fn run_rg_in_vault(model: &AppListModel, vault_path: &Path, pattern: &str) {
    if which("rg").is_some() {
        let mut cmd = std::process::Command::new("rg");
        cmd.arg("-i")
            .arg("--with-filename")
            .arg("--line-number")
            .arg("--no-heading")
            .arg("--color=never")
            .arg(pattern)
            .arg(vault_path);
        run_subprocess(model, cmd);
    } else {
        let mut cmd = std::process::Command::new("grep");
        cmd.arg("-r")
            .arg("-n")
            .arg("-i")
            .arg("-I")
            .arg("-H")
            .arg("--color=never")
            .arg("--")
            .arg(pattern)
            .arg(vault_path);
        run_subprocess(model, cmd);
    }
}
