use std::fs;
use std::path::PathBuf;

use crate::colors::Colors;

/// Create a mock notez directory structure for screenshots and demos.
/// Returns the path to the temp directory.
pub fn run_demo() {
    let dir = std::env::temp_dir().join("notez-demo");

    // Clean previous demo
    if dir.exists() {
        fs::remove_dir_all(&dir).ok();
    }

    // Create project structure
    let project = dir.join("my-project").join("notez");
    let quick = project.join("00_quick-notes");
    let logs = project.join("01_daily-logs");
    let research = project.join("02_research");
    let api = project.join("03_api-design");

    fs::create_dir_all(&quick).unwrap();
    fs::create_dir_all(&logs).unwrap();
    fs::create_dir_all(&research).unwrap();
    fs::create_dir_all(&api).unwrap();

    // Quick notes
    fs::write(
        quick.join("2026-04-01-project-goals.md"),
        "# project-goals\n\nDate: 2026-04-01\n\nDefine the core features and MVP scope.\n",
    ).unwrap();
    fs::write(
        quick.join("2026-04-02-meeting-notes.md"),
        "# meeting-notes\n\nDate: 2026-04-02\n\nDiscussed timeline and priorities with the team.\n",
    ).unwrap();
    fs::write(
        quick.join("2026-04-03-architecture-ideas.md"),
        "# architecture-ideas\n\nDate: 2026-04-03\n\nConsider event-driven vs request-response.\n",
    ).unwrap();

    // Daily logs
    fs::write(
        logs.join("2026-04-01-daily-log.md"),
        "# Daily Log - 2026-04-01\n\n09:15 - Started project setup\n10:30 - Configured CI pipeline\n14:00 - First prototype running\n",
    ).unwrap();
    fs::write(
        logs.join("2026-04-02-daily-log.md"),
        "# Daily Log - 2026-04-02\n\n09:00 - Code review\n11:30 - Fixed auth bug\n15:00 - Deployed to staging\n",
    ).unwrap();
    fs::write(
        logs.join("2026-04-03-daily-log.md"),
        "# Daily Log - 2026-04-03\n\n09:30 - Planning session\n13:00 - Implemented search feature\n",
    ).unwrap();

    // Research
    fs::write(
        research.join("2026-04-01-database-comparison.md"),
        "# database-comparison\n\nDate: 2026-04-01\n\nPostgres vs SQLite for embedded use.\n",
    ).unwrap();
    fs::write(
        research.join("2026-04-02-caching-strategy.md"),
        "# caching-strategy\n\nDate: 2026-04-02\n\nRedis vs in-memory LRU cache.\n",
    ).unwrap();

    // API design
    fs::write(
        api.join("2026-04-02-endpoints.md"),
        "# endpoints\n\nDate: 2026-04-02\n\nGET /items, POST /items, DELETE /items/:id\n",
    ).unwrap();
    fs::write(
        api.join("2026-04-03-auth-flow.md"),
        "# auth-flow\n\nDate: 2026-04-03\n\nOAuth2 with PKCE for the mobile client.\n",
    ).unwrap();

    // TODO.md with subtasks
    fs::write(
        project.join("TODO.md"),
        "# TODO\n\n\
- [/] Set up CI/CD pipeline\n\
  - [x] Configure build workflow\n\
  - [x] Add test runner\n\
  - [ ] Deploy to production\n\
- [ ] Write API documentation\n\
  - [ ] Document endpoints\n\
  - [ ] Add example requests\n\
- [x] Initial project setup\n\
- [ ] Performance benchmarks\n\
- [/] User authentication\n\
  - [x] Login flow\n\
  - [x] Token refresh\n\
  - [ ] Password reset\n\
",
    ).unwrap();

    let colors = Colors::new();

    println!();
    println!(
        "  {} Demo project created at:",
        colors.green.apply_to("✓")
    );
    println!(
        "    {}",
        colors.sapphire.apply_to(project.to_string_lossy())
    );
    println!();
    println!("  Try these commands:");
    println!();
    println!(
        "    {}",
        colors.overlay.apply_to(format!("cd {} && notez tree", dir.join("my-project").to_string_lossy()))
    );
    println!(
        "    {}",
        colors.overlay.apply_to(format!("cd {} && notez todo", dir.join("my-project").to_string_lossy()))
    );
    println!(
        "    {}",
        colors.overlay.apply_to(format!("cd {} && notez -h", dir.join("my-project").to_string_lossy()))
    );
    println!();
}
