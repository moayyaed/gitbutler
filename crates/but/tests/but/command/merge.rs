use snapbox::str;

use crate::utils::{CommandExt, Sandbox};

#[test]
fn merge_first_branch_into_gb_local_and_verify_rebase() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("merge-gb-local-two-branches")?;

    // Run setup to create gb-local remote
    env.but("setup").assert().success();

    // Verify we're on gitbutler/workspace
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()?;
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "gitbutler/workspace"
    );

    // Create first branch
    env.but("branch new first-branch").assert().success();

    // Create first commit on first branch
    env.file("file1.txt", "content1");
    env.but("commit first-branch -m 'first commit on branch A'")
        .assert()
        .success();

    let first_branch = "first-branch";

    // Create second branch with a different commit
    env.but("branch new second-branch").assert().success();

    env.file("file2.txt", "content2");
    env.but("commit second-branch -m 'second commit on branch B'")
        .assert()
        .success();

    // Verify git log shows both branches before merge
    insta::assert_snapshot!(env.git_log()?, @r"
    *   7d1d22f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    * | bc560e9 (first-branch) first commit on branch A
    |/  
    * edca1cd (second-branch) second commit on branch B
    * 85efbe4 (gb-local/main, gb-local/HEAD, main, gitbutler/target) M
    ");

    // Get the current main branch commit (should be the initial commit M)
    let main_before = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("main")
        .output()?;
    let main_before_hash = String::from_utf8_lossy(&main_before.stdout)
        .trim()
        .to_string();

    // Merge the first branch
    env.but(format!("merge {first_branch}"))
        .assert()
        .success()
        .stdout_eq(str![[r#"

Found 3 upstream commits on gb-local/main
   e0e7be6 Merge branch 'first-branch'
   bc560e9 first commit on branch A
   edca1cd second commit on branch B

Updating 2 active branches...


Branch first-branch has been integrated upstream and removed locally
Branch second-branch has been integrated upstream and removed locally

Summary
────────
  first-branch - integrated
  second-branch - integrated

To undo this operation:
  Run `but undo`

"#]]);

    // Verify that main has been updated with the merge commit
    let main_after = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("main")
        .output()?;
    let main_after_hash = String::from_utf8_lossy(&main_after.stdout)
        .trim()
        .to_string();

    // Main should have changed
    assert_ne!(
        main_before_hash, main_after_hash,
        "main branch should have been updated"
    );

    // Verify the merge commit has both parents
    let parents = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-list")
        .arg("--parents")
        .arg("-n")
        .arg("1")
        .arg("main")
        .output()?;
    let parents_str = String::from_utf8_lossy(&parents.stdout);
    let parent_count = parents_str.split_whitespace().count() - 1; // Subtract 1 for the commit itself
    assert_eq!(parent_count, 2, "Merge commit should have 2 parents");

    // Verify file1.txt exists on main now
    let file1_content = std::fs::read_to_string(env.projects_root().join("file1.txt"))?;
    assert_eq!(file1_content, "content1");

    // Verify that both local branches were integrated and removed from the workspace
    let status_after = env
        .but("status --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status_after_str = String::from_utf8_lossy(&status_after);
    let status_after_json: serde_json::Value = serde_json::from_str(&status_after_str)?;

    // Both branches were integrated as upstream commits.
    assert_eq!(
        status_after_json["stacks"].as_array().unwrap().len(),
        0,
        "No local stacks should remain after merge"
    );

    let first_branch_exists = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--verify")
        .arg("first-branch")
        .output()?;
    assert!(
        !first_branch_exists.status.success(),
        "first-branch should be removed after integration"
    );

    let second_branch_exists = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("rev-parse")
        .arg("--verify")
        .arg("second-branch")
        .output()?;
    assert!(
        !second_branch_exists.status.success(),
        "second-branch should be removed after integration"
    );

    // Verify git log shows the integrated structure
    insta::assert_snapshot!(env.git_log()?, @r"
    * 70954b7 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    *   e0e7be6 (gb-local/main, gb-local/HEAD, main) Merge branch 'first-branch'
    |\  
    | | * 7d1d22f (gb-local/gitbutler/workspace) GitButler Workspace Commit
    | |/| 
    |/| | 
    * | | bc560e9 (gb-local/first-branch) first commit on branch A
    | |/  
    |/|   
    * | edca1cd (gb-local/second-branch) second commit on branch B
    |/  
    * 85efbe4 (gb-local/gitbutler/target, gitbutler/target) M
    ");

    Ok(())
}
