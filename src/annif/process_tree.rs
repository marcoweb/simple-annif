//! Keep the CLI and its descendants under one lifetime, without a shell.
use std::io;
use std::process::{Child, Command};

pub struct ProcessTree {
    pub child: Child,
    #[cfg(windows)]
    job: std::os::windows::io::OwnedHandle,
    #[cfg(unix)]
    group: i32,
}

impl ProcessTree {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(windows)]
        {
            windows::spawn(command)
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let child = command.process_group(0).spawn()?;
            Ok(Self {
                group: child.id() as i32,
                child,
            })
        }
    }

    pub fn terminate(&self) -> io::Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            // SAFETY: this owned handle refers to our private job, never another app's.
            if unsafe {
                windows_sys::Win32::System::JobObjects::TerminateJobObject(
                    self.job.as_raw_handle(),
                    1,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        #[cfg(unix)]
        {
            // SAFETY: the group was created specifically for this owned child.
            if unsafe { libc::kill(-self.group, libc::SIGKILL) } != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
            }
        }
        Ok(())
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        // Also close inherited pipes in descendants after the main CLI exits.
        let _ = self.terminate();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::{
                CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
            },
        },
    };

    pub(super) fn spawn(command: &mut Command) -> io::Result<ProcessTree> {
        // SAFETY: unnamed job, no inherited handle, initialized C structures; all
        // handles are checked before ownership transfer and closed through RAII.
        unsafe {
            let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = OwnedHandle::from_raw_handle(raw);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            // Suspend before any user code can create children outside the job.
            // Command retains responsibility for Unicode paths, argv and pipes.
            let child = command
                .creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED)
                .spawn()?;
            let tree = ProcessTree { child, job };
            if AssignProcessToJobObject(tree.job.as_raw_handle(), tree.child.as_raw_handle()) == 0 {
                return Err(io::Error::last_os_error());
            }
            resume_initial_thread(tree.child.id())?;
            Ok(tree)
        }
    }

    fn resume_initial_thread(process_id: u32) -> io::Result<()> {
        // std::process::Child exposes the process handle, not the initial thread.
        // The new process is still suspended and cannot create worker threads.
        // SAFETY: snapshot and thread handles validated; only our child is resumed.
        unsafe {
            let raw = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if raw == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            let snapshot = OwnedHandle::from_raw_handle(raw);
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of_val(&entry) as u32;
            let mut found = Thread32First(snapshot.as_raw_handle(), &mut entry);
            while found != 0 {
                if entry.th32OwnerProcessID == process_id {
                    let raw = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                    if raw.is_null() {
                        return Err(io::Error::last_os_error());
                    }
                    let thread = OwnedHandle::from_raw_handle(raw);
                    if ResumeThread(thread.as_raw_handle()) == u32::MAX {
                        return Err(io::Error::last_os_error());
                    }
                    return Ok(());
                }
                entry.dwSize = std::mem::size_of_val(&entry) as u32;
                found = Thread32Next(snapshot.as_raw_handle(), &mut entry);
            }
        }
        Err(io::Error::other("Thread inicial do Annif não encontrada"))
    }
}
