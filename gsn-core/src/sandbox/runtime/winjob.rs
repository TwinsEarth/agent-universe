//! Windows Job Object（v2.8.7，仅 Windows）
//!
//! 进程级沙箱在 Windows 上的资源与进程树兜底：
//! - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`：job 句柄关闭（含 daemon 崩溃/
//!   正常销毁）时，整棵进程树被系统杀掉，杜绝孤儿；
//! - `JOB_OBJECT_LIMIT_ACTIVE_PROCESS`：限制活动进程数（对应 max_processes）；
//! - `JOB_OBJECT_LIMIT_PROCESS_MEMORY`：限制单进程提交内存。**注意**：高版本
//!   Node/V8 启动即会提交数百 MB，按裸 mem_mb（如 768）设置会令 Node 启动即
//!   abort（退出码 134），因此调用方必须按运行程序计算"有效上限"（Node 与
//!   Linux 的 ulimit -v 对称地留足余量）后传入；Node 的真实 JS 堆另由
//!   `--max-old-space-size` 限制。
//!
//! 主动超时路径另走 taskkill /T /F（见 process.rs）；本模块是**被动**兜底。
#![cfg(windows)]

use std::ffi::c_void;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};

/// 受管的 Job Object；Drop 时关闭句柄并杀整棵进程树
pub struct WinJob {
    handle: HANDLE,
}

impl WinJob {
    /// 创建并配置 job。
    ///
    /// `effective_mem_mb` 是**单进程提交内存上限**，由调用方按运行程序计算：
    /// Node 需在 mem_mb 之上留足启动余量（与 Linux ulimit -v 对称），否则
    /// 高版本 Node 启动即 abort（134）；Python 可直接用 mem_mb。
    /// `max_processes` 是活动进程数上限。
    pub fn new(effective_mem_mb: u32, max_processes: u32) -> Result<Self, String> {
        // SAFETY: CreateJobObjectW 两个参数均为 null（默认安全属性、匿名 job），
        // 该调用本身安全；返回句柄已校验非 0 且非 INVALID_HANDLE_VALUE。
        // SetInformationJobObject 传入的 info 是全字段显式初始化的
        // JOBOBJECT_EXTENDED_LIMIT_INFORMATION，指针/长度与类型匹配，
        // 仅在当前线程调用；失败时显式 CloseHandle 释放句柄，不泄漏。
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job == 0 || job == INVALID_HANDLE_VALUE {
                return Err("CreateJobObject 失败".into());
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            info.BasicLimitInformation.ActiveProcessLimit = max_processes;
            // 单进程提交内存上限（字节）
            info.ProcessMemoryLimit = (effective_mem_mb as usize).saturating_mul(1024 * 1024);

            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                CloseHandle(job);
                return Err("SetInformationJobObject 失败".into());
            }
            Ok(WinJob { handle: job })
        }
    }

    /// 把子进程（及其后由它创建的子进程）纳入 job
    pub fn assign(&self, process_handle: HANDLE) -> Result<(), String> {
        // SAFETY: self.handle 是 new 成功返回的有效 job 句柄；process_handle
        // 由调用方保证为已创建子进程的有效句柄。此处仅检查返回值，不解引用
        // 任何裸指针，也不持有该句柄的所有权（不会重复关闭）。
        unsafe {
            if AssignProcessToJobObject(self.handle, process_handle) == 0 {
                return Err("AssignProcessToJobObject 失败".into());
            }
        }
        Ok(())
    }
}

impl Drop for WinJob {
    fn drop(&mut self) {
        // SAFETY: self.handle 在 WinJob 生命周期内始终有效（由 new 成功保证），
        // CloseHandle 仅在 Drop 中调用一次，之后结构体不再被使用。
        // 关闭即因 JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE 由系统终止整棵进程树。
        unsafe {
            // 关闭即因 KILL_ON_JOB_CLOSE 杀掉整棵进程树
            CloseHandle(self.handle);
        }
    }
}
