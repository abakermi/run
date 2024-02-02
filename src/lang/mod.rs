mod bash;
mod c;
mod cpp;
mod csharp;
mod javascript;
mod python;
mod rust;
mod shell;

pub use bash::Bash;
pub use c::C;
pub use cpp::Cpp;
pub use csharp::CSharp;
pub use javascript::Javascript;
pub use python::Python;
pub use rust::Rust;
pub use shell::Shell;

use yansi::Paint as _;

use crate::fmt::Str;

#[derive(Clone, Copy, PartialEq, Eq)]
#[enum_dispatch::enum_dispatch]
pub enum Lang {
    Shell,
    Bash,
    Rust,
    Python,
    Javascript,
    C,
    Cpp,
    CSharp,
}

#[enum_dispatch::enum_dispatch(Lang)]
pub trait Language {
    fn as_str(&self) -> &'static str;
    fn binary(&self) -> &'static str;
    fn nix_packages(&self) -> &'static [&'static str];
    fn execute(&self, input: &str) -> Result<(), Str<'_>> {
        execute_interpreted(self.program()?, input)
    }
    fn installed(&self) -> bool {
        which::which(self.binary()).is_ok()
    }
    fn program(&self) -> Result<std::process::Command, Str<'static>> {
        which::which(self.binary())
            .map(std::process::Command::new)
            .map_err(|error| exe_not_found(self.binary(), error))
            .or_else(|error| crate::nix::nix_shell(self.nix_packages(), self.binary()).ok_or(error))
    }
}

pub(crate) fn exe_not_found(exe: &str, error: which::Error) -> Str<'_> {
    let purple = yansi::Color::BrightMagenta.bold();
    let not_found =
        "executable could not be found.\nDo you have it installed and in the PATH?\n\nRun '";
    let run = "run --commands".bright_cyan().bold();
    let for_more = "' for more information.".paint(purple);
    let error = format!(
        "{}'{exe}' {not_found}{run}{for_more}\n\nComplete error: {error}",
        "".paint(purple).linger()
    );
    Str::from(error)
}

pub(crate) fn execution_failed(
    exe: impl std::fmt::Display,
    error: impl std::fmt::Display,
) -> Str<'static> {
    let error = format!(
        "{}'{exe}' failed to execute command{}\n\nComplete error: {error}",
        "".bright_magenta().bold().linger(),
        "".clear()
    );
    Str::from(error)
}

fn write_to_tmp(dir: &str, input: &str) -> Result<std::path::PathBuf, Str<'static>> {
    let to_error = |e: std::io::Error| Str::from(e.to_string());

    // Write to file to allow inheriting stdin
    let file = std::env::temp_dir().join("run/").join(dir);
    std::fs::create_dir_all(&file).map_err(to_error)?;
    let file = file.join("input");
    std::fs::write(&file, input).map_err(to_error)?;
    Ok(file)
}

fn wait_for_child(mut child: std::process::Child) -> Result<(), Str<'static>> {
    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(Str::from(format!(
            "Command exited with status code {}",
            status.code().unwrap_or(-1)
        ))),
        Err(e) => Err(Str::from(format!(
            "Failed to wait for command to exit: {}",
            e
        ))),
    }
}

/// Runs the given program with only one argument consisting in a file containing the input.
///
/// ```
/// execute_simple("python", "print('Hello')");
/// ```
/// Is equivalent to
/// ```bash
/// echo "print('Hello')" > /tmp/run/input && python /tmp/run/input
/// ```
fn execute_interpreted(
    mut program: std::process::Command,
    input: &str,
) -> Result<(), Str<'static>> {
    let name = format!("{:?}", program.get_program());
    let file = write_to_tmp("input", input).unwrap();
    let child = program
        .arg(file)
        .spawn()
        .map_err(|error| execution_failed(name, error))?;
    wait_for_child(child)
}

fn create_project(
    name: &str,
    init: Option<&mut std::process::Command>,
    main: impl AsRef<std::path::Path>,
    input: &str,
) -> Result<std::path::PathBuf, Str<'static>> {
    static APP_INFO: app_dirs2::AppInfo = app_dirs2::AppInfo {
        name: "runfile",
        author: "lyonsyonii",
    };
    let main = main.as_ref();

    let path = format!("cache/{name}/{:x}", md5::compute(input));
    if std::path::Path::new(&path).exists() {
        return Ok(path.into());
    }

    let Ok(path) = app_dirs2::app_dir(app_dirs2::AppDataType::UserCache, &APP_INFO, &path) else {
        return Err("Could not create project directory".into());
    };

    if let Some(init) = init {
        init.current_dir(&path)
            .output()
            .map_err(|error| execution_failed(init.get_program().to_string_lossy(), error))?;
    }

    std::fs::write(path.join(main), input)
        .map_err(|e| format!("Could not write input to {path:?}/{main:?}\nComplete error: {e}"))?;

    Ok(path)
}

fn execute_compiled(
    lang: &str,
    proj_main: impl AsRef<std::path::Path>,
    input: &str,
    init: Option<&mut std::process::Command>,
    compile: &mut std::process::Command,
    run: &mut std::process::Command,
) -> Result<(), Str<'static>> {
    let path = create_project(lang, init, proj_main, input)?;
    std::env::set_current_dir(&path)
        .map_err(|e| format!("Could not set current directory to {path:?}\nComplete error: {e}"))?;

    let compile = compile
        .output()
        .map_err(|error| execution_failed(compile.get_program().to_string_lossy(), error))?;

    if !compile.status.success() {
        let err = String::from_utf8(compile.stderr)
            .map_err(|_| "Failed to parse command output as UTF-8")?;
        return Err(Str::from(err));
    }

    let child = run
        .spawn()
        .map_err(|error| execution_failed(run.get_program().to_string_lossy(), error))?;

    wait_for_child(child)
}

impl std::str::FromStr for Lang {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cmd" | "fn" | "sh" | "shell" => Ok(Shell.into()),
            "bash" => Ok(Bash.into()),
            "rs" | "rust" => Ok(Rust.into()),
            "c" => Ok(C.into()),
            "c++" | "cpp" | "cplusplus" => Ok(Cpp.into()),
            "c#" | "cs" | "csharp" => Ok(CSharp.into()),
            "py" | "python" => Ok(Python.into()),
            "js" | "javascript" => Ok(Javascript.into()),
            _ => Err(s.to_owned()),
        }
    }
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::fmt::Debug for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for Lang {
    fn default() -> Self {
        Shell.into()
    }
}
