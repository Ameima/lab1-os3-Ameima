use crate::config::*;
use crate::trap::TrapContext;

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [KernelStack {
    data: [0; KERNEL_STACK_SIZE],
}; MAX_APP_NUM];

static USER_STACK: [UserStack; MAX_APP_NUM] = [UserStack {
    data: [0; USER_STACK_SIZE],
}; MAX_APP_NUM];

impl KernelStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }
    pub fn push_context(&self, trap_cx: TrapContext) -> usize {
        let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *trap_cx_ptr = trap_cx;
        }
        trap_cx_ptr as usize
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

// 计算各个应用的加载基址
fn get_base_i(app_id: usize) -> usize {
    // 每个应用的加载基址为总基址 + 应用的顺位id * 每个应用固定分配的大小区域
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

// 获取个数数值
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

// 加载应用
pub fn load_apps() {
    // 引入符号，这个符号来自link_app.S，是一个存着应用总个数的地址
    extern "C" {
        fn _num_app();
    }
    // 转换成裸指针
    let num_app_ptr = _num_app as usize as *const usize;
    // 获取应用数数值
    let num_app = get_num_app();

    // 用刚才得到的裸指针和应用数数值生成切片，里面有所有的应用的符号（编译时转换为各个应用的地址，见link_app.S）
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };

    // 清理i-cache，直接用内联汇编指令
    unsafe {
        core::arch::asm!("fence.i");
    }

    // 用迭代器加载应用
    for i in 0..num_app {
        // 获取各个应用被加载到的基址
        let base_i = get_base_i(i);
        // 先清零区域，从应用基址清理应用固定的大小
        (base_i..base_i + APP_SIZE_LIMIT)
            .for_each(|addr| unsafe { (addr as *mut u8).write_volatile(0) });
        // 应用源地址，相当于外存
        let src = unsafe {
            core::slice::from_raw_parts(app_start[i] as *const u8, app_start[i + 1] - app_start[i])
        };
        // 应用目标地址，相当于内存
        let dst = unsafe { core::slice::from_raw_parts_mut(base_i as *mut u8, src.len()) };
        // 装载
        dst.copy_from_slice(src);
    }
}

pub fn init_app_cx(app_id: usize) -> usize {
    KERNEL_STACK[app_id].push_context(TrapContext::app_init_context(
        get_base_i(app_id),
        USER_STACK[app_id].get_sp(),
    ))
}
