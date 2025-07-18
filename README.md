# S7 Event Monitor

This application receives data packets from a Siemens PLC (via a TSEND), and stores them in a database.
The intention is that the PLC will send us event data when certain things happen, and we have a timestamped log for diagnostics.

## Communication Specification

*TODO*

The application can receive up to 512 bytes.
Packets sent from the PLC can be any length less than or equal to that 512 bytes, and they will be fully received by the application.
Other functions within the app expect a particular format which is described below.

It is expected that all data is 4 bytes in size. A `DINT` in the PLC, which equates to a `u32` in rust.

The structure is:

| Bytes  | Decription             |
|--------|------------------------|
| 0-3    | Event Code             |
| 4-7    | PLC Packet Code        |
| 8..511 | Per-packet custom data |

## How to use the Appication (WIP)

*TODO*

## Programming the PLC

There is a TIA V15.1 sample project inside this project. It shows how to setup the comms sending, and the triggering of events.

## Installing the Development Environment

*a.k.a. Getting Started With Rust (For Dummies)*

### 1. Install a linker

Rust has its own compiler, but you will need a linker to create Windows applications.
Visual studio is possibly the simplest to get going, but the GNU toolchain (gcc) also works.

If you do use the Visual Studio linker, install Visual Studio Community Edition with these options:

- C++ Desktop apps

### 2. Install the Rust Compiler

Download `rustup-init.exe` from [https://rustup.rs/](https://rustup.rs/) and run it.

Follow the on screen prompts. If you installed Visual Studio, select the option for that (should be option 1), and you will be more or less ready to go.

Log out and back in, or restart your PC, so that the `cargo` command is in your path environment variable.

### 3. Setup an IDE

The recommended IDE is [Visual Studio Code](https://code.visualstudio.com/) (**not** Visual Studio), with some extensions.

Install the *rust-analyzer* extension, which will install other extensions that you need.

### 4. Build the application

The easiest way is to go to the root of the application's directory and run `cargo run`.

You can also do `cargo build` if you just want to build the application without running it.

Learn more about rust at [https://www.rust-lang.org/](https://www.rust-lang.org/).
