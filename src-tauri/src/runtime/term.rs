//! 交互式会话——在用户自己的终端里开一个窗口。
//!
//! 为什么不由启动器自己承载会话：这六个 Agent 的交互界面都是 TUI，要一个真的 PTY 和一个
//! 终端仿真器才能跑；webview 里没有，硬做就是在启动器里再写一个终端。启动器的既有分工也
//! 正是这样——dsh 的 web 界面交给浏览器，日志视图只读，Agent 自己的界面从不由启动器托管。
//! 所以「交互式」= 把这次启动写成一个脚本，让用户默认的终端去执行它。
//!
//! 终端的探测规则与 [`crate::engines::detect_engines`] 一字不差：只查 PATH，从不执行候选，
//! 不落缓存；`$TERMINAL` 优先，那是用户自己定的答案。
//!
//! 只做 Unix。Windows 上没有 `sh`，而 macOS 的 `open -a Terminal` 立刻返回、启动器就再也
//! supervise 不了那个会话——两种都得单独设计，没验证过的东西不摆在这里。

use std::path::{Path, PathBuf};

use crate::engines::find_on_path;

/// 候选终端，以及「要执行的命令」前面得垫的那几个参数。
///
/// 顺序即优先级：先 Wayland/现代仿真器，再各桌面自带的，最后 X11 老将。`kitty` 与 `foot`
/// 把命令直接当尾随参数收，所以是空切片；其余都要一个开关，而那个开关每家都不一样——GTK
/// 一系要 `--`，Xfce/terminator 要 `-x`，其余多半沿用 xterm 的 `-e`。
const TERMINALS: &[(&str, &[&str])] = &[
    ("kitty", &[]),
    ("foot", &[]),
    ("alacritty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("ghostty", &["-e"]),
    ("konsole", &["-e"]),
    ("gnome-terminal", &["--"]),
    ("kgx", &["--"]),
    ("xfce4-terminal", &["-x"]),
    ("terminator", &["-x"]),
    ("tilix", &["-e"]),
    ("deepin-terminal", &["-e"]),
    ("qterminal", &["-e"]),
    ("lxterminal", &["-e"]),
    ("mate-terminal", &["--"]),
    ("urxvt", &["-e"]),
    ("st", &["-e"]),
    ("xterm", &["-e"]),
    // Debian/Ubuntu 的 alternatives 名，指向上面某一个；放最后当兜底。
    ("x-terminal-emulator", &["-e"]),
];

/// 表里没有的 `$TERMINAL` 假定吃什么开关。`-e` 是 xterm 传下来、最多人沿用的那个——是个
/// 猜测，也正是 `$TERMINAL` 要先拿去查表的原因。
const UNKNOWN_ARGS: &[&str] = &["-e"];

/// 一个能开窗的终端：可执行文件的绝对路径，加上命令前要垫的参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terminal {
    /// 绝对路径。`$TERMINAL` 给的是裸名字时也在 PATH 上解析成绝对路径，理由与
    /// `EngineInfo::path` 相同：用户要看得见启动器到底挑中了哪一个。
    pub program: String,
    pub args: Vec<String>,
}

/// 表里查一个名字该垫什么参数；查不到用 [`UNKNOWN_ARGS`]。
fn args_for(stem: &str) -> Vec<String> {
    TERMINALS
        .iter()
        .find(|(bin, _)| *bin == stem)
        .map(|(_, a)| *a)
        .unwrap_or(UNKNOWN_ARGS)
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// 把一个名字或路径解析成 [`Terminal`]。带分隔符的当路径用（存在才算），否则查 PATH。
fn resolve(name_or_path: &str, path_var: &str) -> Option<Terminal> {
    let raw = name_or_path.trim();
    if raw.is_empty() {
        return None;
    }
    let p = Path::new(raw);
    let stem = p.file_name()?.to_string_lossy().into_owned();
    let program = if raw.contains(std::path::MAIN_SEPARATOR) {
        p.is_file().then(|| raw.to_string())?
    } else {
        find_on_path(raw, path_var)?
    };
    Some(Terminal {
        program,
        args: args_for(&stem),
    })
}

/// 挑一个终端：先 `$TERMINAL`（用户自己定的答案），再按 [`TERMINALS`] 的顺序找第一个装着的。
///
/// `path_var` 用执行器算给子进程的那份 PATH，不是启动器自己的——从图标启动的 GUI 拿到的
/// PATH 是被裁过的，那正是 [`super::env`] 存在的原因。
pub fn pick(path_var: &str) -> Option<Terminal> {
    if let Ok(t) = std::env::var("TERMINAL") {
        if let Some(found) = resolve(&t, path_var) {
            return Some(found);
        }
    }
    TERMINALS.iter().find_map(|(bin, _)| resolve(bin, path_var))
}

/// 找不到终端时说给用户听的那句话——把 `$TERMINAL` 这条出路一起说清楚，否则用户只知道
/// 「失败了」，不知道装什么或者设什么。
pub fn not_found_message() -> String {
    let names: Vec<&str> = TERMINALS.iter().take(6).map(|(b, _)| *b).collect();
    format!(
        "找不到可用的终端。装一个（{}…），或者把 $TERMINAL 设成你惯用的那个。",
        names.join(" / ")
    )
}

/// 把一个值包成 `sh` 的单引号字面量：里面除了单引号本身没有任何字符还有意义，而单引号
/// 用 `'\''`（收尾、转义一个、再开头）接回去。启动脚本里每一个插进去的值都要过这里——
/// 模型名、路径、密钥都是外来文本，拼进 shell 就得当成会咬人的东西。
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// 写出这次交互式启动的脚本，返回它的路径。
///
/// 为什么走脚本而不是直接 `terminal -e agent --flag …`：
///   * 环境变量能确实到位。gnome-terminal / konsole 一类是 D-Bus 客户端，新窗口继承的是
///     那个常驻服务的环境，不是启动器的——把 export 写进脚本，这条坑就不存在了。
///   * 引号只需要对一次。每家终端把 `-e` 后面的东西怎么再切一遍各有说法，一个路径参数
///     没有歧义。
///   * 用户能自己看、自己跑。启动器到底会执行什么，`run.sh` 就是答案。
///
/// 文件里有这次启动要用的环境变量，其中包括 API Key，所以权限是 0700——和同目录的 `.env`
/// 同一条规矩，每次启动重写。
pub fn write_run_script(
    inst_dir: &Path,
    workspace: &Path,
    title: &str,
    envs: &[(String, String)],
    program: &str,
    args: &[String],
) -> Result<PathBuf, String> {
    let mut s = String::from(
        "#!/bin/sh\n\
         # 由 agentLauncher 生成，每次启动都会重写——想改就去编辑实例，别改这里。\n\
         # 含本次启动的环境变量（可能有 API Key），权限 0700，与同目录的 .env 同规矩。\n",
    );
    s.push_str(&format!(
        "cd {} || exit 1\n",
        sh_quote(&workspace.to_string_lossy())
    ));
    for (k, v) in envs {
        // 变量名不引；它来自 `.env` 与服务商表，不是 shell 表达式的位置。值一律引。
        s.push_str(&format!("export {}={}\n", k, sh_quote(v)));
    }
    // 窗口标题写实例名，这样开着好几个会话时任务栏上认得出谁是谁。
    s.push_str(&format!(
        "printf '\\033]0;%s\\007' {}\n",
        sh_quote(&format!("{title} — agentLauncher"))
    ));
    s.push_str(&sh_quote(program));
    for a in args {
        s.push(' ');
        s.push_str(&sh_quote(a));
    }
    s.push('\n');
    // 不用 exec：留住这层 sh，退出码非零时把窗口按住。交互式会话正常退出就该跟着关掉窗口
    // （和你自己在终端里退出一样），但启动失败的那一行必须让人看见——所有终端的 --hold
    // 开关都不一样，这里用一次 read 把这件事做成统一的。
    s.push_str(
        "code=$?\n\
         if [ \"$code\" -ne 0 ]; then\n  \
           printf '\\n[agentLauncher] 退出码 %s。按回车关闭窗口…' \"$code\"\n  \
           read -r _\n\
         fi\n\
         exit \"$code\"\n",
    );

    let path = inst_dir.join("run.sh");
    std::fs::write(&path, s).map_err(|e| format!("写启动脚本失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("设置启动脚本权限失败: {e}"))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_tree;

    #[test]
    fn a_single_quote_in_a_value_cannot_escape_the_string() {
        assert_eq!(sh_quote("plain"), "'plain'");
        // 关键的一条：值里带单引号也不能让后面的东西变成命令。
        assert_eq!(sh_quote("a'b"), r"'a'\''b'");
        assert_eq!(sh_quote("$(rm -rf /)"), "'$(rm -rf /)'");
        assert_eq!(sh_quote("a b"), "'a b'");
    }

    #[test]
    fn known_terminals_get_their_own_flag_and_unknown_ones_get_dash_e() {
        assert!(args_for("kitty").is_empty(), "kitty 直接收尾随参数");
        assert_eq!(args_for("alacritty"), ["-e"]);
        assert_eq!(args_for("wezterm"), ["start", "--"]);
        assert_eq!(args_for("gnome-terminal"), ["--"]);
        assert_eq!(args_for("xfce4-terminal"), ["-x"]);
        // 表里没有的：按最常见的约定猜 `-e`。
        assert_eq!(args_for("some-new-terminal"), ["-e"]);
    }

    #[test]
    fn a_terminal_is_resolved_to_an_absolute_path_on_the_given_path() {
        let tree = temp_tree("term-pick");
        let bin_dir = tree.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let kitty = bin_dir.join("kitty");
        std::fs::write(&kitty, "#!/bin/sh\n").unwrap();

        let found = resolve("kitty", &bin_dir.to_string_lossy()).expect("found on PATH");
        assert_eq!(found.program, kitty.to_string_lossy());
        assert!(found.args.is_empty());

        // 不在这份 PATH 上就是没有——不去别处扫盘。
        assert_eq!(resolve("kitty", "/nonexistent"), None);
        // 绝对路径直接用，但文件得真的在。
        assert_eq!(
            resolve(&kitty.to_string_lossy(), "/nonexistent").map(|t| t.program),
            Some(kitty.to_string_lossy().into_owned())
        );
        assert_eq!(resolve("/nope/nope/kitty", ""), None);
    }

    #[test]
    fn the_script_quotes_every_value_and_holds_the_window_only_on_failure() {
        let tree = temp_tree("term-script");
        let path = write_run_script(
            tree.path(),
            &tree.path().join("work space"),
            "克劳德特工",
            &[("MY_KEY".into(), "sk-a'b".into())],
            "omp",
            &["--model".into(), "nvidia/llama'x".into()],
        )
        .unwrap();
        let body = std::fs::read_to_string(&path).unwrap();

        assert!(body.starts_with("#!/bin/sh\n"), "{body}");
        assert!(body.contains("cd '"), "工作目录带空格也得引起来: {body}");
        assert!(body.contains(r"export MY_KEY='sk-a'\''b'"), "{body}");
        assert!(
            body.contains(r"'omp' '--model' 'nvidia/llama'\''x'"),
            "{body}"
        );
        // 正常退出就让窗口跟着关，失败才按住。
        assert!(body.contains("if [ \"$code\" -ne 0 ]"), "{body}");
        assert!(body.contains("read -r _"), "{body}");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "脚本里有密钥，权限只能是 0700");
        }
    }

    #[test]
    fn the_not_found_message_names_a_way_out() {
        let msg = not_found_message();
        assert!(msg.contains("$TERMINAL"), "{msg}");
        assert!(msg.contains("kitty"), "{msg}");
    }
}
