use std::fs;

use crate::colors::Colors;

/// Create a mock notez directory structure for screenshots and demos.
pub fn run_demo(_view: Option<String>) {
    let dir = dirs::home_dir().unwrap().join(".notez-demo");

    // Clean previous demo
    if dir.exists() {
        fs::remove_dir_all(&dir).ok();
    }

    // --- Project 1: my-project (private .notez + public notez) ---
    let p1_private = dir.join("my-project").join(".notez");
    let p1_public = dir.join("my-project").join("notez");
    let p1_quick = p1_private.join("00_quick-notes");
    let p1_logs = p1_private.join("01_daily-logs");
    let p1_research = p1_private.join("02_research");

    for d in [&p1_quick, &p1_logs, &p1_research] {
        fs::create_dir_all(d).unwrap();
    }
    fs::create_dir_all(&p1_public.join("00_quick-notes")).unwrap();

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

    // Private TODO with tags and nested subtasks
    fs::write(p1_private.join("TODO.md"),
        "# TODO\n\n- [/] Set up CI/CD pipeline #important #prio\n  - [x] Configure build workflow\n  - [x] Add test runner\n  - [ ] Deploy to production #blocked\n    - [ ] Set up staging env\n    - [ ] Write deploy script\n- [ ] Write API documentation #longterm\n  - [ ] Document endpoints\n    - [ ] GET /items\n    - [ ] POST /items\n  - [ ] Add example requests\n- [x] Initial project setup\n- [/] User authentication #prio\n  - [x] Login flow\n  - [x] Token refresh\n  - [ ] Password reset #idea\n").unwrap();

    // Public TODO with subtasks
    fs::write(p1_public.join("TODO.md"),
        "# TODO\n\n- [ ] Update contributing guide\n  - [ ] Code of conduct\n  - [ ] PR template\n- [x] Add license file\n- [ ] Improve README #prio\n  - [/] Add install instructions\n  - [ ] Add usage examples\n").unwrap();

    fs::write(p1_public.join("00_quick-notes").join("2026-04-03-onboarding.md"),
        "# onboarding\n\nDate: 2026-04-03\n\nSteps for new contributors.\n").unwrap();

    // Tags for tree
    fs::write(p1_private.join(".tags"),
        "02_research:4\n").unwrap(); // idea flag on research dir

    // --- Project 2: webapp (private only) ---
    let p2 = dir.join("webapp").join(".notez");
    let p2_quick = p2.join("00_quick-notes");
    fs::create_dir_all(&p2_quick).unwrap();

    fs::write(p2_quick.join("2026-04-02-design-system.md"),
        "# design-system\n\nDate: 2026-04-02\n\nComponent library with Tailwind.\n").unwrap();

    fs::write(p2.join("TODO.md"),
        "# TODO\n\n- [ ] Design system components #idea\n  - [x] Button variants\n  - [ ] Form inputs\n  - [ ] Modal dialogs\n- [/] Landing page #prio\n  - [x] Hero section\n  - [ ] Pricing table\n- [ ] Dark mode support #longterm\n").unwrap();

    // --- Global notez home (simulated) ---
    let home = dir.join("notez-home");
    let home_quick = home.join("00_quick-notes");
    let home_logs = home.join("01_daily-logs");
    fs::create_dir_all(&home_quick).unwrap();
    fs::create_dir_all(&home_logs).unwrap();

    // Symlink private project dirs into home
    let home_p1 = home.join("02_my-project");
    let home_p2 = home.join("03_webapp");
    std::os::unix::fs::symlink(&p1_private, &home_p1).ok();
    std::os::unix::fs::symlink(&p2, &home_p2).ok();

    fs::write(home_quick.join("2026-04-01-personal-ideas.md"),
        "# personal-ideas\n\nDate: 2026-04-01\n\nRandom thoughts and inspiration.\n").unwrap();

    fs::write(home_logs.join("2026-04-03-daily-log.md"),
        "# Daily Log - 2026-04-03\n\n08:00 - Morning review\n12:00 - Lunch walk\n").unwrap();

    fs::write(home.join("TODO.md"),
        "# TODO\n\n- [ ] Review quarterly goals #important\n- [x] Update dotfiles\n- [ ] Read Rust book chapter 12 #longterm\n").unwrap();

    // Write a temporary config pointing to the demo home
    let demo_config_dir = dir.join("notez");
    fs::create_dir_all(&demo_config_dir).unwrap();
    let demo_config = demo_config_dir.join("config");
    let config_content = format!(
        "NOTEZ_ROOT={}\nQUICK_NOTES_DIR=00_quick-notes\nDAILY_LOGS_DIR=01_daily-logs\nEDITOR=nvim\nHAS_FZF=true\nHAS_RG=true\nHAS_YAZI=true\n",
        home.to_string_lossy()
    );
    fs::write(&demo_config, config_content).unwrap();

    // Project mapping for global view to find public notes
    let projects_file = demo_config_dir.join("projects");
    fs::write(&projects_file, format!(
        "my-project={}\nwebapp={}\n",
        dir.join("my-project").to_string_lossy(),
        dir.join("webapp").to_string_lossy(),
    )).unwrap();

    let colors = Colors::new();
    let cd_p1 = format!("cd {}", dir.join("my-project").to_string_lossy());
    let xdg = format!("XDG_CONFIG_HOME={}", dir.to_string_lossy());

    println!();
    println!(
        "  {} Demo created with 2 projects + global notes",
        colors.green.apply_to("✓")
    );
    println!();
    println!("  {}", colors.mauve.apply_to("Local view (project)"));
    println!(
        "    {}",
        colors.overlay.apply_to(format!("{} && notez tree", cd_p1))
    );
    println!(
        "    {}",
        colors.overlay.apply_to(format!("{} && todoz", cd_p1))
    );
    println!();
    println!("  {}", colors.mauve.apply_to("Global view (all projects)"));
    println!(
        "    {}",
        colors.overlay.apply_to(format!("{} {} notez -g tree", cd_p1, xdg))
    );
    println!(
        "    {}",
        colors.overlay.apply_to(format!("{} {} todoz -g", cd_p1, xdg))
    );
    println!();
    println!("  {}", colors.mauve.apply_to("Help"));
    println!(
        "    {}",
        colors.overlay.apply_to("notez -h")
    );
    println!();
}
