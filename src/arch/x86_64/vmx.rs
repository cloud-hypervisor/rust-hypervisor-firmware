// SPDX-License-Identifier: Apache-2.0

use core::arch::x86_64::__cpuid;
use x86_64::registers::model_specific::Msr;

const IA32_FEATURE_CONTROL: u32 = 0x3a;
const FEATURE_CONTROL_LOCKED: u64 = 1 << 0;
const FEATURE_CONTROL_VMXON_OUTSIDE_SMX: u64 = 1 << 2;
const CPUID_1_ECX_VMX: u32 = 1 << 5;

// A guest hypervisor (Hyper-V / WSL2) refuses VMXON unless firmware locked IA32_FEATURE_CONTROL with VMXON enabled; real BIOS does this at boot, so we must too or nested virt is unusable despite VMX in CPUID.
pub fn enable_feature_control() {
    let has_vmx = unsafe { __cpuid(1) }.ecx & CPUID_1_ECX_VMX != 0;
    if !has_vmx {
        return;
    }
    let mut msr = Msr::new(IA32_FEATURE_CONTROL);
    let current = unsafe { msr.read() };
    if current & FEATURE_CONTROL_LOCKED == 0 {
        unsafe { msr.write(current | FEATURE_CONTROL_VMXON_OUTSIDE_SMX | FEATURE_CONTROL_LOCKED) };
    }
}
