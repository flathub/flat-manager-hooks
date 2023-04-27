use std::error::Error;

use log::info;
use ostree::{gio::Cancellable, glib, glib::GString, MutableTree, Repo};

pub fn get_job_id() -> Result<i64, Box<dyn Error>> {
    Ok(std::env::var("FLAT_MANAGER_JOB_ID")?.parse()?)
}

pub fn get_build_id() -> Result<i64, Box<dyn Error>> {
    Ok(std::env::var("FLAT_MANAGER_BUILD_ID")?.parse()?)
}

pub fn arch_from_ref(refstring: &str) -> String {
    refstring.split('/').nth(2).unwrap().to_string()
}

pub fn app_id_from_ref(refstring: &str) -> String {
    let ref_id = refstring.split('/').nth(1).unwrap().to_string();
    let id_parts: Vec<&str> = ref_id.split('.').collect();

    if ["Sources", "Debug", "Locale"].contains(id_parts.last().unwrap()) {
        id_parts[..id_parts.len() - 1].to_vec().join(".")
    } else {
        ref_id
    }
}

pub fn mtree_lookup(
    mtree: &MutableTree,
    path: &[&str],
) -> Result<(Option<GString>, Option<MutableTree>), Box<dyn Error>> {
    match path {
        [file] => mtree.lookup(file).map_err(Into::into),
        [subdir, rest @ ..] => mtree_lookup(
            &mtree
                .lookup(subdir)?
                .1
                .ok_or_else(|| "subdirectory not found".to_string())?,
            rest,
        ),
        [] => Err("no path given".into()),
    }
}

pub fn mtree_lookup_file(mtree: &MutableTree, path: &[&str]) -> Result<GString, Box<dyn Error>> {
    mtree_lookup(mtree, path)?
        .0
        .ok_or_else(|| "file not found".into())
}

/// Wrapper for OSTree transactions that automatically aborts the transaction when dropped if it hasn't been committed.
pub struct Transaction<'a> {
    repo: &'a Repo,
    finished: bool,
}

impl<'a> Transaction<'a> {
    pub fn new(repo: &'a Repo) -> Result<Self, glib::Error> {
        repo.prepare_transaction(Cancellable::NONE)?;
        Ok(Self {
            repo,
            finished: false,
        })
    }

    pub fn commit(mut self) -> Result<(), glib::Error> {
        self.repo.commit_transaction(Cancellable::NONE)?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.repo
                .abort_transaction(Cancellable::NONE)
                .expect("Aborting the transaction should not fail");
        }
    }
}

/// Try the given retry function up to `retry_count + 1` times. The first successful result is returned, or the last error if all attempts failed.
pub fn retry<T, E: std::fmt::Display, F: Fn() -> Result<T, E>>(f: F) -> Result<T, E> {
    let mut i = 0;

    let retry_count = 5;
    let mut wait_time = 1;

    loop {
        match f() {
            Ok(info) => return Ok(info),
            Err(e) => {
                info!("{}", e);
                i += 1;
                if i > retry_count {
                    return Err(e);
                }
                info!("Retrying ({i}/{retry_count}) in {wait_time} seconds...");
                std::thread::sleep(std::time::Duration::from_secs(wait_time));
                wait_time *= 2;
            }
        }
    }
}
