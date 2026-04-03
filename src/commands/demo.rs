use std::fs;

use crate::colors::Colors;
use crate::config::Config;

/// Create a mock notez directory structure for screenshots and demos.
pub fn run_demo() {
    let dir = dirs::home_dir().unwrap().join(".notez-demo");

    // Clean previous demo
    if dir.exists() {
        fs::remove_dir_all(&dir).ok();
    }

    // --- Project 1: my-project ---
    let p1 = dir.join("my-project").join("notez");
    let p1_quick = p1.join("00_quick-notes");
    let p1_logs = p1.join("01_daily-logs");
    let p1_research = p1.join("02_research");
    let p1_api = p1.join("03_api-design");

    for d in [&p1_quick, &p1_logs, &p1_research, &p1_api] {
        fs::create_dir_all(d).unwrap();
    }

    fs::write(p1_quick.join("2026-04-01-project-goals.md"),
        "# project-goals\n\nDate: 2026-04-01\n\nDefine the core features and MVP scope.\n").unwrap();
    fs::write(p1_quick.join("2026-04-02-meeting-notes.md"),
        "# meeting-notes\n\nDate: 2026-04-02\n\nDiscussed timeline and priorities with the team.\n").unwrap();
    fs::write(p1_quick.join("2026-04-03-architecture-ideas.md"),
        "# architecture-ideas\n\nDate: 2026-04-03\n\nConsider event-driven vs request-response.\n").unwrap();

    fs::write(p1_logs.join("2026-04-01-daily-log.md"),
        "# Daily Log - 2026-04-01\n\n09:15 - Started project setup\n10:30 - Configured CI pipeline\n14:00 - First prototype running\n").unwrap();
    fs::write(p1_logs.join("2026-04-02-daily-log.md"),
        "# Daily Log - 2026-04-02\n\n09:00 - Code review\n11:30 - Fixed auth bug\n15:00 - Deployed to staging\n").unwrap();

    fs::write(p1_research.join("2026-04-01-database-comparison.md"),
        "# database-comparison\n\nDate: 2026-04-01\n\nPostgres vs SQLite for embedded use.\n").unwrap();
    fs::write(p1_api.join("2026-04-02-endpoints.md"),
        "# endpoints\n\nDate: 2026-04-02\n\nGET /items, POST /items, DELETE /items/:id\n").unwrap();

    fs::write(p1.join("TODO.md"),
        "# TODO\n\n\
- [/] Set up CI/CD pipeline\n\
  - [x] Configure build workflow\n\
  - [x] Add test runner\n\
  - [ ] Deploy to production\n\
- [ ] Write API documentation\n\
  - [ ] Document endpoints\n\
  - [ ] Add example requests\n\
- [x] Initial project setup\n\
- [/] User authentication\n\
  - [x] Login flow\n\
  - [x] Token refresh\n\
  - [ ] Password reset\n").unwrap();

    // --- Project 2: webapp ---
    let p2 = dir.join("webapp").join("notez");
    let p2_quick = p2.join("00_quick-notes");
    fs::create_dir_all(&p2_quick).unwrap();

    fs::write(p2_quick.join("2026-04-02-design-system.md"),
        "# design-system\n\nDate: 2026-04-02\n\nComponent library with Tailwind.\n").unwrap();

    fs::write(p2.join("TODO.md"),
        "# TODO\n\n\
- [ ] Design system components\n\
  - [x] Button variants\n\
  - [ ] Form inputs\n\
  - [ ] Modal dialogs\n\
- [/] Landing page\n\
  - [x] Hero section\n\
  - [ ] Pricing table\n\
- [ ] Dark mode support\n").unwrap();

    // --- Global notez home (simulated) ---
    let home = dir.join("notez-home");
    let home_quick = home.join("00_quick-notes");
    let home_logs = home.join("01_daily-logs");
    fs::create_dir_all(&home_quick).unwrap();
    fs::create_dir_all(&home_logs).unwrap();

    // Symlink projects into home
    let home_p1 = home.join("02_my-project");
    let home_p2 = home.join("03_webapp");
    std::os::unix::fs::symlink(&p1, &home_p1).ok();
    std::os::unix::fs::symlink(&p2, &home_p2).ok();

    fs::write(home_quick.join("2026-04-01-personal-ideas.md"),
        "# personal-ideas\n\nDate: 2026-04-01\n\nRandom thoughts and inspiration.\n").unwrap();

    fs::write(home_logs.join("2026-04-03-daily-log.md"),
        "# Daily Log - 2026-04-03\n\n08:00 - Morning review\n12:00 - Lunch walk\n").unwrap();

    fs::write(home.join("TODO.md"),
        "# TODO\n\n\
- [ ] Review quarterly goals\n\
- [x] Update dotfiles\n\
- [ ] Read Rust book chapter 12\n").unwrap();

    // Write a temporary config pointing to the demo home
    // XDG_CONFIG_HOME looks for $XDG_CONFIG_HOME/notez/config
    let demo_config_dir = dir.join("notez");
    fs::create_dir_all(&demo_config_dir).unwrap();
    let demo_config = demo_config_dir.join("config");
    let config_content = format!(
        "NOTEZ_ROOT={}\nQUICK_NOTES_DIR=00_quick-notes\nDAILY_LOGS_DIR=01_daily-logs\nEDITOR=nvim\nHAS_FZF=true\nHAS_RG=true\nHAS_YAZI=true\n",
        home.to_string_lossy()
    );
    fs::write(&demo_config, config_content).unwrap();

    let colors = Colors::new();
    let cd_p1 = format!("cd {}", dir.join("my-project").to_string_lossy());

    println!();
    println!(
        "  {} Demo created with 2 projects + global notes",
        colors.green.apply_to("✓")
    );
    println!();
    println!("  {}", colors.mauve.apply_to("Project view"));
    println!(
        "    {}",
        colors.overlay.apply_to(format!("{} && notez tree", cd_p1))
    );
    println!(
        "    {}",
        colors.overlay.apply_to(format!("{} && notez todo", cd_p1))
    );
    println!();
    println!("  {}", colors.mauve.apply_to("Global view (all projects)"));
    println!(
        "    {}",
        colors.overlay.apply_to(format!(
            "XDG_CONFIG_HOME={} notez -g tree",
            dir.to_string_lossy()
        ))
    );
    println!(
        "    {}",
        colors.overlay.apply_to(format!(
            "XDG_CONFIG_HOME={} notez -g todo",
            dir.to_string_lossy()
        ))
    );
    println!(
        "    {}",
        colors.overlay.apply_to(format!(
            "XDG_CONFIG_HOME={} todoz -g",
            dir.to_string_lossy()
        ))
    );
    println!();
    println!("  {}", colors.mauve.apply_to("Help"));
    println!(
        "    {}",
        colors.overlay.apply_to("notez -h")
    );
    println!();
}
