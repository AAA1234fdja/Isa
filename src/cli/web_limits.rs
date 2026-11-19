//! Optional resource restrictions for processes launched by the web supervisor.
//! These controls are not a sandbox or a security boundary against hostile code.
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Profile { Host, Limited }
impl Profile {
    pub(crate) fn configured() -> Result<Self, &'static str> {
        match std::env::var("ISA_WEB_EXEC_PROFILE").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("host") => Ok(Self::Host),
            Ok("limited") if cfg!(target_os="linux") => Ok(Self::Limited),
            _ => Err("Invalid or unsupported execution profile; local actions are disabled."),
        }
    }
    pub(crate) fn name(self) -> &'static str { match self { Self::Host => "host", Self::Limited => "limited" } }
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
