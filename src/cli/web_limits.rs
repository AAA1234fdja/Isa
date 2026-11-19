//! Optional resource restrictions for processes launched by the web supervisor.
//! The limited profile is not a sandbox. The sandboxed profile uses bubblewrap
//! with host networking; its isolation depends on the OS and configuration.
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Profile { Host, Limited, Sandboxed }
impl Profile {
    pub(crate) fn configured() -> Result<Self, &'static str> {
        match std::env::var("ISA_WEB_EXEC_PROFILE").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("host") => Ok(Self::Host),
            Ok("limited") if cfg!(target_os="linux") => Ok(Self::Limited),
            Ok("sandboxed") if cfg!(target_os="linux") && std::path::Path::new("/usr/bin/bwrap").is_file() => Ok(Self::Sandboxed),
            _ => Err("Invalid or unsupported execution profile; local actions are disabled."),
        }
    }
    pub(crate) fn name(self) -> &'static str { match self { Self::Host => "host", Self::Limited => "limited", Self::Sandboxed => "sandboxed" } }
}

/// Build the web child command. The sandbox profile requires bubblewrap and
/// never falls back to host execution. The workspace is mounted at /workspace;
/// trusted runtime directories are readable, and host networking remains enabled.
pub(crate) fn command(root: &std::path::Path, args: &[String], profile: Profile) -> std::io::Result<Command> {
    let Some(program)=args.first() else { return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,"empty command")); };
    // Reject unsupported profiles before constructing a command, even when a
    // caller forgets to invoke configure(). Never silently return a host command.
    if !cfg!(target_os="linux") && profile != Profile::Host {
        return Err(std::io::Error::new(std::io::ErrorKind::Unsupported,"execution profile requires Linux"));
    }
    if profile != Profile::Sandboxed { let mut cmd=Command::new(program);cmd.args(&args[1..]).current_dir(root);return Ok(cmd); }
    #[cfg(target_os="linux")]
    {
        if root == std::path::Path::new("/") || !std::path::Path::new("/usr/bin/bwrap").is_file() {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied,"sandbox is unavailable or workspace is unsafe"));
        }
        let mut cmd=Command::new("/usr/bin/bwrap");
        cmd.args(["--die-with-parent","--new-session","--unshare-user","--unshare-pid","--unshare-ipc","--unshare-uts","--cap-drop","ALL"]);
        for path in ["/usr","/bin","/lib","/lib64","/etc/resolv.conf","/etc/hosts","/etc/ssl/certs"] { cmd.args(["--ro-bind-try",path,path]); }
        cmd.args(["--proc","/proc","--dev","/dev","--tmpfs","/tmp","--bind"]);cmd.arg(root).arg("/workspace");
        cmd.args(["--chdir","/workspace","--"]);cmd.args(args);cmd.current_dir(root);Ok(cmd)
    }
    #[cfg(not(target_os="linux"))]
    { let _=(root,args);Err(std::io::Error::new(std::io::ErrorKind::Unsupported,"sandbox requires Linux")) }
}

/// Configure only web-launched processes. Errors applying limits cause spawn to
/// fail; do not fall back to the unrestricted profile.
pub(crate) fn configure(command: &mut Command, profile: Profile) -> std::io::Result<()> {
    if profile == Profile::Host { return Ok(()); }
    #[cfg(target_os="linux")]
    {
        use std::os::unix::process::CommandExt;
        // Fixed minimal environment, not a confidentiality boundary: the process
        // still has the same user's filesystem and process permissions.
        command.env_clear().env("PATH", "/usr/bin:/bin").env("LANG", "C.UTF-8");
        // All operations in the forked child are async-signal-safe syscalls;
        // never relax a limit already imposed by the parent.
        unsafe { command.pre_exec(|| {
            for (resource, requested) in [
                (libc::RLIMIT_CORE, 0 as libc::rlim_t),
                (libc::RLIMIT_CPU, 30 as libc::rlim_t),
                (libc::RLIMIT_AS, 4_000_000_000u64 as libc::rlim_t),
                (libc::RLIMIT_FSIZE, 33_554_432 as libc::rlim_t),
                (libc::RLIMIT_NOFILE, 128 as libc::rlim_t),
            ] {
                let mut current=std::mem::MaybeUninit::<libc::rlimit>::uninit();
                if libc::getrlimit(resource,current.as_mut_ptr()) != 0 { return Err(std::io::Error::last_os_error()); }
                let current=current.assume_init();
                let value=requested.min(current.rlim_cur).min(current.rlim_max);
                let limit=libc::rlimit { rlim_cur:value, rlim_max:value };
                if libc::setrlimit(resource,&limit) != 0 { return Err(std::io::Error::last_os_error()); }
            }
            Ok(())
        }); }
        Ok(())
    }
    #[cfg(not(target_os="linux"))]
    { let _=command; Err(std::io::Error::new(std::io::ErrorKind::Unsupported,"limited profile requires Linux")) }
}

#[cfg(all(test,target_os="linux"))]
mod tests {
    use super::*;
    #[test]
    fn sandbox_is_fail_closed_when_binary_is_missing() {
        if !std::path::Path::new("/usr/bin/bwrap").is_file() {
            assert!(command(std::path::Path::new("/tmp"),&["/bin/true".into()],Profile::Sandboxed).is_err());
        }
        assert!(command(std::path::Path::new("/"),&["/bin/true".into()],Profile::Sandboxed).is_err());
    }
    #[test]
    fn sandbox_mounts_workspace_and_keeps_network_when_enabled() {
        if std::env::var_os("ISA_SANDBOX_TEST").is_none() { return; }
        use std::net::TcpListener;
        let base=std::env::temp_dir().join(format!("isa-sandbox-test-{}",std::process::id()));
        let workspace=base.join("workspace");std::fs::create_dir_all(&workspace).unwrap();
        let secret=base.join("host-secret");std::fs::write(&secret,"secret").unwrap();
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
        let shell=format!("test ! -e '{}' && echo ok > /workspace/written && : > /dev/tcp/127.0.0.1/{port}",secret.display());
        let args=vec!["/bin/bash".into(),"-c".into(),shell];let mut child=command(&workspace,&args,Profile::Sandboxed).unwrap();configure(&mut child,Profile::Sandboxed).unwrap();
        let output=child.output().unwrap();assert!(output.status.success(),"sandbox failed: {}",String::from_utf8_lossy(&output.stderr));assert_eq!(std::fs::read_to_string(workspace.join("written")).unwrap().trim(),"ok");
        std::fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(all(test, windows))]
mod windows_fail_closed_tests {
    use super::*;
    #[test]
    fn windows_fail_closed_profiles_never_construct_or_configure_a_command() {
        let args = vec!["cmd.exe".to_string()];
        for profile in [Profile::Limited, Profile::Sandboxed] {
            assert_eq!(command(std::path::Path::new("."), &args, profile).unwrap_err().kind(), std::io::ErrorKind::Unsupported);
            assert_eq!(configure(&mut Command::new("cmd.exe"), profile).unwrap_err().kind(), std::io::ErrorKind::Unsupported);
        }
    }
}
