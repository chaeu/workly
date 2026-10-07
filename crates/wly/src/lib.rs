//! `wly`: the Workly CLI for coding agents and humans. A thin layer over
//! workly-core; output field names are documented in docs/CLI.md.
//!
//! Lives in a lib so the app binary can run it too (Settings → Install CLI
//! links `~/.local/bin/wly` to the app, which dispatches on argv[0]).

use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use workly_core::agent::{self, AgentFile};
use workly_core::project::is_workspace;
use workly_core::scan::{ProjectEntry, TaskEntry};
use workly_core::settings::{self, Settings, expand_home};
use workly_core::{Error, Workspace};

/// Workly CLI. Agents change task state only through it.
#[derive(Parser)]
#[command(name = "wly", version = workly_core::VERSION)]
struct Cli {
    /// Workspace folder [default: active workspace from the Workly settings]
    #[arg(long, global = true, env = "WORKLY_WORKSPACE", value_name = "PATH")]
    workspace: Option<String>,
    /// Acting agent; logged as agent:<NAME> instead of cli
    #[arg(long, global = true, env = "WORKLY_AGENT", value_name = "NAME")]
    agent: Option<String>,
    /// JSON on stdout, JSON errors on stderr
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(subcommand)]
    Task(TaskCmd),
    #[command(subcommand)]
    Project(ProjectCmd),
}

#[derive(Subcommand)]
enum TaskCmd {
    /// List tasks
    List {
        /// Project key, e.g. WR
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    /// Task, project and agent context (global rules, project AGENTS.md, skills)
    Show { id: String },
    /// Set doing (from backlog, todo or review); with --agent also agent.active and agent.since
    Start { id: String },
    /// Append a line under ## Updates
    Note {
        id: String,
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
    },
    /// Set review (from doing) and clear agent.active
    Review {
        id: String,
        #[arg(long, value_name = "SHA")]
        commit: Option<String>,
    },
    /// Create a task; prints its id
    Add {
        title: String,
        /// Project key [default: inbox]
        #[arg(long, conflicts_with = "inbox")]
        project: Option<String>,
        #[arg(long)]
        inbox: bool,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
        priority: Option<u8>,
    },
    /// Set done. Humans only: refused with exit 3 when an agent is set
    Done { id: String },
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// List projects
    List,
}

// ------------------------------------------------------------------ output

#[derive(Serialize)]
struct TaskOut {
    id: String,
    title: String,
    status: String,
    priority: Option<u8>,
    due: Option<String>,
    tags: Vec<String>,
    /// Project key; null for inbox tasks.
    project: Option<String>,
    path: PathBuf,
    created: Option<String>,
    done_at: Option<String>,
    agent: Option<AgentOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
}

#[derive(Serialize)]
struct AgentOut {
    ready: bool,
    runner: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    active: Option<String>,
    since: Option<String>,
    commit: Option<String>,
    last_run: Option<String>,
}

#[derive(Serialize)]
struct ProjectOut {
    key: String,
    title: String,
    status: String,
    path: PathBuf,
    repos: Vec<String>,
}

#[derive(Serialize)]
struct FileOut {
    path: PathBuf,
    content: String,
}

#[derive(Serialize)]
struct SkillOut {
    name: String,
    description: String,
    path: PathBuf,
}

#[derive(Serialize)]
struct ContextOut {
    global: Option<FileOut>,
    project: Option<FileOut>,
    skills: Vec<SkillOut>,
}

fn task_out(ws: &Workspace, t: &TaskEntry) -> TaskOut {
    let t2 = &t.task;
    TaskOut {
        id: t2.id.clone(),
        title: t2.title.clone(),
        status: t2.status.clone(),
        priority: t2.priority,
        due: t2.due.clone(),
        tags: t2.tags.clone(),
        project: project_of(ws, t).map(|p| p.project.key.clone()),
        path: ws.root().join(&t.path),
        created: t2.created.clone(),
        done_at: t2.done_at.clone(),
        agent: t2.agent.as_ref().map(|a| AgentOut {
            ready: a.ready,
            runner: a.runner.clone(),
            model: a.model.clone(),
            effort: a.effort.clone(),
            active: a.active.clone(),
            since: a.since.clone(),
            commit: a.commit.clone(),
            last_run: a.last_run.clone(),
        }),
        body: None,
    }
}

fn project_out(ws: &Workspace, p: &ProjectEntry) -> ProjectOut {
    let q = &p.project;
    ProjectOut { key: q.key.clone(), title: q.title.clone(), status: q.status.clone(), path: ws.root().join(&p.path), repos: q.repos.clone() }
}

fn project_of<'a>(ws: &'a Workspace, t: &TaskEntry) -> Option<&'a ProjectEntry> {
    let folder = t.project.as_deref()?;
    ws.index.projects.iter().find(|p| p.path == folder)
}

fn print_json(v: &impl Serialize) {
    println!("{}", serde_json::to_string_pretty(v).expect("serialisable"));
}

// --------------------------------------------------------------------- run

/// Run the CLI with `args` (argv[0] first). Returns the exit code:
/// 0 ok, 1 usage error, 2 not found, 3 invalid transition.
pub fn run(args: impl IntoIterator<Item = OsString>) -> i32 {
    let args: Vec<OsString> = args.into_iter().collect();
    let json = args.iter().any(|a| a == "--json");
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) if !e.use_stderr() => {
            let _ = e.print();
            return 0;
        }
        Err(e) if json => {
            let msg = e.to_string();
            let first = msg.lines().next().unwrap_or_default().trim_start_matches("error: ");
            return fail(true, &Error::Invalid(first.to_string()));
        }
        Err(e) => {
            let _ = e.print();
            return 1;
        }
    };
    match exec(&cli) {
        Ok(()) => 0,
        Err(e) => fail(cli.json, &e),
    }
}

fn fail(json: bool, e: &Error) -> i32 {
    let code = match e {
        Error::Invalid(_) | Error::Conflict(_) | Error::Io(_) => 1,
        Error::NotFound(_) => 2,
        Error::InvalidTransition(_) => 3,
    };
    if json {
        eprintln!("{}", json!({ "error": { "code": code, "message": e.to_string() } }));
    } else {
        eprintln!("wly: {e}");
    }
    code
}

/// --workspace, then WORKLY_WORKSPACE (both via clap), then the app's active workspace.
fn open_workspace(cli: &Cli) -> workly_core::Result<Workspace> {
    let path = match cli.workspace.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => expand_home(p),
        None => {
            let s = Settings::load(&settings::default_path())?;
            let active = s.active.ok_or_else(|| Error::Invalid("no workspace: pass --workspace, set WORKLY_WORKSPACE or pick one in the app".into()))?;
            PathBuf::from(active)
        }
    };
    if !is_workspace(&path) {
        return Err(Error::NotFound(format!("{} is not a Workly workspace (no .workly/config.yml)", path.display())));
    }
    Workspace::open(&path)
}

fn exec(cli: &Cli) -> workly_core::Result<()> {
    let agent = cli.agent.as_deref().filter(|a| !a.is_empty());
    if let Some(name) = agent {
        agent::check_name(name)?;
    }
    let actor = agent.map_or("cli".to_string(), |a| format!("agent:{a}"));
    let mut ws = open_workspace(cli)?;
    match &cli.cmd {
        Cmd::Project(ProjectCmd::List) => {
            let list: Vec<ProjectOut> = ws.index.projects.iter().map(|p| project_out(&ws, p)).collect();
            if cli.json {
                print_json(&list);
            } else {
                for p in &list {
                    println!("{:<6} {:<9} {}", p.key, p.status, p.title);
                }
            }
        }
        Cmd::Task(TaskCmd::List { project, status }) => {
            let folder = match project {
                Some(key) => Some(ws.index.project(key).ok_or_else(|| Error::NotFound(format!("project {key} not found")))?.path.clone()),
                None => None,
            };
            if let Some(s) = status
                && !ws.config.task_statuses.iter().any(|x| &x.id == s)
            {
                return Err(Error::Invalid(format!("unknown status {s}")));
            }
            let list: Vec<TaskOut> = (ws.index.tasks.iter())
                .filter(|t| folder.is_none() || t.project == folder)
                .filter(|t| status.as_ref().is_none_or(|s| &t.task.status == s))
                .map(|t| task_out(&ws, t))
                .collect();
            if cli.json {
                print_json(&list);
            } else {
                for t in &list {
                    let prio = t.priority.map_or(String::new(), |p| format!("P{p}"));
                    let working = t.agent.as_ref().and_then(|a| a.active.as_deref()).map_or(String::new(), |a| format!("  ({a} working)"));
                    println!("{:<8} {:<7} {:<3} {}{working}", t.id, t.status, prio, t.title);
                }
            }
        }
        Cmd::Task(TaskCmd::Show { id }) => show(&ws, id, cli.json)?,
        Cmd::Task(TaskCmd::Start { id }) => {
            ws.start_task(id, agent, &actor)?;
            done_msg(&ws, id, cli.json);
        }
        Cmd::Task(TaskCmd::Note { id, text }) => {
            ws.add_task_update(id, &text.join(" "), &actor)?;
            done_msg(&ws, id, cli.json);
        }
        Cmd::Task(TaskCmd::Review { id, commit }) => {
            ws.review_task(id, commit.as_deref(), &actor)?;
            done_msg(&ws, id, cli.json);
        }
        Cmd::Task(TaskCmd::Done { id }) => {
            ws.update_task_field(id, "status", &Value::from("done"), &actor)?;
            done_msg(&ws, id, cli.json);
        }
        Cmd::Task(TaskCmd::Add { title, project, inbox: _, priority }) => {
            let id = ws.create_task(title, project.as_deref(), *priority, &actor)?;
            if cli.json {
                print_json(&task_out(&ws, ws.index.task(&id).unwrap()));
            } else {
                println!("{id}");
            }
        }
    }
    Ok(())
}

/// After a write: the task as JSON, or one line `WR-5 doing`.
fn done_msg(ws: &Workspace, id: &str, json: bool) {
    let t = ws.index.task(id).expect("just written");
    if json {
        print_json(&task_out(ws, t));
    } else {
        println!("{id} {}", t.task.status);
    }
}

fn show(ws: &Workspace, id: &str, json: bool) -> workly_core::Result<()> {
    let t = ws.index.task(id).ok_or_else(|| Error::NotFound(format!("task {id} not found")))?;
    let project = project_of(ws, t);
    let ctx = ws.agent_context(project.map(|p| p.project.key.as_str()))?;
    let mut task = task_out(ws, t);
    task.body = Some(ws.read_markdown(&t.path)?);
    let abs = |p: &str| ws.root().join(p);
    let file = |f: Option<AgentFile>| f.map(|f| FileOut { path: abs(&f.path), content: f.content });
    let context = ContextOut {
        global: file(ctx.global),
        project: file(ctx.project),
        skills: ctx.skills.into_iter().map(|s| SkillOut { name: s.name, description: s.description, path: abs(&s.path) }).collect(),
    };
    let project = project.map(|p| project_out(ws, p));
    if json {
        print_json(&json!({ "task": task, "project": project, "context": context }));
        return Ok(());
    }
    print!("{}", show_text(&task, project.as_ref(), &context));
    Ok(())
}

/// `task show` as Markdown: what an agent reads before it starts.
fn show_text(t: &TaskOut, p: Option<&ProjectOut>, ctx: &ContextOut) -> String {
    let mut out = format!("# {} · {}\n\n", t.id, t.title);
    let mut meta = vec![format!("status: {}", t.status)];
    meta.extend(t.priority.map(|p| format!("priority: {p}")));
    meta.extend(t.due.as_ref().map(|d| format!("due: {d}")));
    if !t.tags.is_empty() {
        meta.push(format!("tags: {}", t.tags.join(", ")));
    }
    out += &format!("{}\n", meta.join(" · "));
    match p {
        Some(p) => {
            out += &format!("project: {} · {}\n", p.key, p.title);
            if !p.repos.is_empty() {
                out += &format!("repos: {}\n", p.repos.join(", "));
            }
        }
        None => out += "project: none (inbox)\n",
    }
    out += &format!("file: {}\n\n", t.path.display());
    out += t.body.as_deref().unwrap_or_default().trim();
    out += "\n";
    let section = |title: &str, f: &Option<FileOut>| match f {
        Some(f) => format!("\n---\n\n## {title} ({})\n\n{}\n", f.path.display(), f.content.trim()),
        None => String::new(),
    };
    out += &section("Workspace rules", &ctx.global);
    out += &section("Project instructions", &ctx.project);
    if !ctx.skills.is_empty() {
        out += "\n---\n\n## Skills (read the file before using one)\n\n";
        for s in &ctx.skills {
            out += &format!("- {}: {} ({})\n", s.name, s.description, s.path.display());
        }
    }
    out
}

/// True when the process was started as `wly` (e.g. through the symlink from Install CLI).
pub fn invoked_as_wly(arg0: &Path) -> bool {
    arg0.file_name().is_some_and(|n| n == "wly")
}
