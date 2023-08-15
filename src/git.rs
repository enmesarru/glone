use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;

use git2::{Cred, RemoteCallbacks, Repository};

use crate::config::{AuthType, Provider};

pub fn download(provider: &Provider, m: &MultiProgress) {
    let repo_path = Path::new(&provider.sync_dir);

    if repo_path.exists() && repo_path.is_dir() {
        pull(provider, repo_path, &m);
    } else {
        clone(provider, repo_path, &m);
    }
}

fn pull(provider: &Provider, repo_path: &Path, m: &MultiProgress) {
    let repo = match Repository::open(repo_path) {
        Ok(repo) => repo,
        Err(e) => {
            log::error!("Failed to open the repository {:?} {}", repo_path, e);
            panic!("Failed to open the repository {:?} {}", repo_path, e)
        }
    };

    let mut remote = repo.find_remote("origin").unwrap();
    let fetch_commit = fetch(&repo, &[&provider.branch], &mut remote, &m).unwrap();

    let _ = merge(&repo, &provider.branch, fetch_commit);
}

fn clone(provider: &Provider, repo_path: &Path, m: &MultiProgress) {
    let mut callbacks = RemoteCallbacks::new();

    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )
    .unwrap()
    .progress_chars("##-");

    let n = 100;
    let pb = m.add(ProgressBar::new(n));
    pb.set_style(sty.clone());
    pb.set_message(format!("{}", &provider.name));

    match provider.auth.r#type {
        AuthType::Token => {
            if provider.auth.is_valid_cred() {
                callbacks.credentials(|_url, _username_from_url, _allowed_types| {
                    Cred::userpass_plaintext(
                        &provider.auth.get_username(),
                        &provider.auth.get_password(),
                    )
                });
            }
        }
        AuthType::Ssh => {
            callbacks.credentials(|_url, username_from_url, _allowed_types| {
                Cred::ssh_key(
                    username_from_url.unwrap(),
                    None,
                    Path::new(provider.auth.get_ssh_path()),
                    None,
                )
            });
        }
        AuthType::Public => {}
    }

    callbacks.transfer_progress(move |stats| {
        let total = stats.total_objects().try_into().unwrap();
        let received: u64 = stats.received_objects().try_into().unwrap();
        pb.set_length(total);

        if received == total {
            pb.finish_with_message("finished");
        } else {
            pb.inc(1);
        }

        true
    });

    // Prepare fetch options.
    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    // Prepare builder.
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    // Clone the project.
    let _ = builder
        .branch(&provider.branch)
        .clone(&provider.url, repo_path);
}

fn fetch<'a>(
    repo: &'a git2::Repository,
    refs: &[&str],
    remote: &'a mut git2::Remote,
    m: &MultiProgress,
) -> Result<git2::AnnotatedCommit<'a>, git2::Error> {
    let mut cb = git2::RemoteCallbacks::new();

    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )
    .unwrap()
    .progress_chars("##-");

    let n = 100;
    let pb = m.add(ProgressBar::new(n));
    pb.set_style(sty.clone());
    pb.set_message(format!("{}", &remote.name().unwrap()));

    // Print out our transfer progress.
    cb.transfer_progress(move |stats| {
        let total = stats.total_objects().try_into().unwrap();
        let received: u64 = stats.received_objects().try_into().unwrap();
        pb.set_length(total);

        if received == total {
            pb.finish_with_message("finished");
        } else {
            pb.inc(1);
        }

        true
    });

    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(cb);

    fo.download_tags(git2::AutotagOption::All);
    log::info!("Fetching {} for repo", remote.name().unwrap());
    remote.fetch(refs, Some(&mut fo), None)?;

    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    Ok(repo.reference_to_annotated_commit(&fetch_head)?)
}

fn fast_forward(
    repo: &Repository,
    lb: &mut git2::Reference,
    rc: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = match lb.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    log::info!("{}", msg);
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
    Ok(())
}

fn normal_merge(
    repo: &Repository,
    local: &git2::AnnotatedCommit,
    remote: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let local_tree = repo.find_commit(local.id())?.tree()?;
    let remote_tree = repo.find_commit(remote.id())?.tree()?;
    let ancestor = repo
        .find_commit(repo.merge_base(local.id(), remote.id())?)?
        .tree()?;
    let mut idx = repo.merge_trees(&ancestor, &local_tree, &remote_tree, None)?;

    if idx.has_conflicts() {
        println!("Merge conflicts detected...");
        repo.checkout_index(Some(&mut idx), None)?;
        return Ok(());
    }
    let result_tree = repo.find_tree(idx.write_tree_to(repo)?)?;
    // now create the merge commit
    let msg = format!("Merge: {} into {}", remote.id(), local.id());
    let sig = repo.signature()?;
    let local_commit = repo.find_commit(local.id())?;
    let remote_commit = repo.find_commit(remote.id())?;
    // Do our merge commit and set current branch head to that commit.
    let _merge_commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &msg,
        &result_tree,
        &[&local_commit, &remote_commit],
    )?;
    // Set working tree to match head.
    repo.checkout_head(None)?;
    Ok(())
}

fn merge<'a>(
    repo: &'a Repository,
    remote_branch: &str,
    fetch_commit: git2::AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    let analysis = repo.merge_analysis(&[&fetch_commit])?;

    if analysis.0.is_fast_forward() {
        log::info!("Doing a fast forward");
        let refname = format!("refs/heads/{}", remote_branch);
        match repo.find_reference(&refname) {
            Ok(mut r) => {
                fast_forward(repo, &mut r, &fetch_commit)?;
            }
            Err(_) => {
                // The branch doesn't exist so just set the reference to the
                // commit directly. Usually this is because you are pulling
                // into an empty repository.
                repo.reference(
                    &refname,
                    fetch_commit.id(),
                    true,
                    &format!("Setting {} to {}", remote_branch, fetch_commit.id()),
                )?;
                repo.set_head(&refname)?;
                repo.checkout_head(Some(
                    git2::build::CheckoutBuilder::default()
                        .allow_conflicts(true)
                        .conflict_style_merge(true)
                        .force(),
                ))?;
            }
        };
    } else if analysis.0.is_normal() {
        let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
        normal_merge(&repo, &head_commit, &fetch_commit)?;
    } else {
        log::info!("There is nothing to do...");
    }
    Ok(())
}
