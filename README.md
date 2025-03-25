# Unclad
An experimental kernel framework, with the goal of providing Rust applications a declarative and deterministic runtime environment for high performance applications

## Status
Unclad has only just started development, and little effort has been put into any of the goals below. Unclad's current goal is to get a basic kernel running on a narrow set of x86_64 implementations.

Once the kernel is in a place where it can reliably run with a usable build process, work will begin on the below goals

## Goals
- Provide high performance, compile-time determined memory management
- Ability to target exact hardware specifications for lean implementation and high-determinism memory layouts
- Enable composable compilation for 'Only what you need' artifacts
- Hands-off runtime. The kernel will have little to no runtime post-loading
- Allow async runtimes to poll as little as possible by providing direct access to hardware and software events
- Target a wide set of 64bit architectures: x86_64, aarch64, riscv64
## Non goals
- user-mode processes. Unclad is meant for trusted environments with robust error handling
- 32bit support. Unclad is meant high-performance, modern hardware. It is not intended for embedded hardware in it's current stage
- Complete `std` implementation. No explicit file system support is planned. Although parts of std will be implemented, large parts will remain un-supported
- Multitasking. Unclad is meant to allow developers to implement application specific operating environments. Async will be very well supported in place of processes
