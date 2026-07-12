//! Executor:core 内所有系统副作用的唯一出口。
//! core 只构造 Cmd(意图),由 Executor 执行 —— 测试用 FakeExecutor 注入预设结果。

use tracing::{debug, warn};

use std::collections::HashMap;
use std::path::PathBuf;

use crate::Error;

/// 一条待执行的命令(纯数据,可断言、可序列化)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    /// 可选工作目录。
    pub cwd: Option<PathBuf>,
}

impl Cmd {
    pub fn new(program: impl Into<String>) -> Self {
        Cmd {
            program: program.into(),
            args: vec![],
            envs: vec![],
            cwd: None,
        }
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.args.push(a.into());
        self
    }

    pub fn args<I, S>(mut self, it: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(it.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.envs.push((k.into(), v.into()));
        self
    }

    /// 用于单测断言:命令的简短可读形式。
    pub fn display(&self) -> String {
        let mut s = self.program.clone();
        for a in &self.args {
            s.push(' ');
            s.push_str(a);
        }
        s
    }
}

/// 执行结果。
#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ExecOutput {
    pub fn success(stdout: impl Into<String>) -> Self {
        ExecOutput {
            status: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    pub fn failure(status: i32, stderr: impl Into<String>) -> Self {
        ExecOutput {
            status,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

/// 执行器抽象。
pub trait Executor {
    fn run(&self, cmd: &Cmd) -> Result<ExecOutput, Error>;
}

/// 真实执行器:调用 std::process::Command。
pub struct RealExecutor;

impl Executor for RealExecutor {
    fn run(&self, cmd: &Cmd) -> Result<ExecOutput, Error> {
        debug!(target: "vagent::exec", cmd = %cmd.display(), "exec");
        use std::process::Command;
        let mut c = Command::new(&cmd.program);
        c.args(&cmd.args);
        for (k, v) in &cmd.envs {
            c.env(k, v);
        }
        if let Some(cwd) = &cmd.cwd {
            c.current_dir(cwd);
        }
        let out = c.output()?;
        let status = out.status.code().unwrap_or(-1);
        if status != 0 {
            warn!(
                target: "vagent::exec",
                cmd = %cmd.display(),
                status,
                stderr = %String::from_utf8_lossy(&out.stderr).trim(),
                "command exited non-zero"
            );
        }
        Ok(ExecOutput {
            status,
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }
}

/// 测试执行器:按 program 返回预设输出,并记命令历史供断言。
#[derive(Default)]
pub struct FakeExecutor {
    pub script: HashMap<String, ExecOutput>,
    pub history: Vec<Cmd>,
}

impl FakeExecutor {
    pub fn new() -> Self {
        FakeExecutor::default()
    }

    pub fn expect(mut self, program: impl Into<String>, out: ExecOutput) -> Self {
        self.script.insert(program.into(), out);
        self
    }
}

impl Executor for FakeExecutor {
    fn run(&self, cmd: &Cmd) -> Result<ExecOutput, Error> {
        // 记录历史(需内部可变;用 RefCell 简化)
        HISTORY.with(|h| h.borrow_mut().push(cmd.clone()));
        match self.script.get(&cmd.program) {
            Some(out) => Ok(out.clone()),
            None => Ok(ExecOutput::success("")),
        }
    }
}

// 历史记录需要跨调用累积(测试用),用线程局部存储。
thread_local! {
    static HISTORY: std::cell::RefCell<Vec<Cmd>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// 取并清空测试历史。
pub fn take_history() -> Vec<Cmd> {
    HISTORY.with(|h| {
        let mut g = h.borrow_mut();
        std::mem::take(&mut *g)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_display_formats_args() {
        let c = Cmd::new("vagent")
            .arg("render")
            .args(["--config", "/etc/vagent/spec.toml"]);
        assert_eq!(c.display(), "vagent render --config /etc/vagent/spec.toml");
    }

    #[test]
    fn fake_executor_returns_scripted() {
        let ex = FakeExecutor::new().expect("acme.sh", ExecOutput::success("issued"));
        let out = ex.run(&Cmd::new("acme.sh").arg("issue")).unwrap();
        assert!(out.ok());
        assert_eq!(out.stdout, "issued");
    }

    #[test]
    fn fake_executor_records_history() {
        take_history();
        let ex = FakeExecutor::new();
        let _ = ex.run(&Cmd::new("systemctl").args(["restart", "xray"]));
        let h = take_history();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].program, "systemctl");
        assert_eq!(h[0].args, vec!["restart", "xray"]);
    }
}
