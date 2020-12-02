# WebMMB Server

WebMMB server provides the backend for [WebMMB](https://github.com/MadCatX/WebMMB) application. WebMMB server requires functional [MMB](https://github.com/samuelflores/MMB) installation.

## Installation
WebMMBServer uses [Rocket](https://rocket.rs/) which requires the latest Rust nightly toolchain. Follow the instructions at [rustup.rs](https://rustup.rs/) to set up Rust development toolchain. Install the `nightly` toolchain by issuing

    rustup toolchain install nightly

command in the terminal. `cd` into the directory with WebMMBServer project sources and set the `nightly` toolchain as default for the project with the following command:

    rustup override set nightly

Compile the project with the following command:

    cargo build --release

By default, project will be built in `target/release` subdirectory as `web_mmb_server` executable.

## Configuration

___NOTE: This is a temporary solution and will change in the future___

WebMMBServer needs a configuration file to launch. A `cfg.json` file needs to be present in the directory WebMMBServer is launched from. The configuration file has a standard JSON file structure, the individual values are described below.

* `mmb_exec_path`: Path to the MMB executable
* `mmb_parameters_file`: Path to the "parameters.csv" file needed by the MMB
* `jobs_dir`: Directory where WebMMBServer will store MMB job data
* `root_dir`: Directory with the WebMMB web application data
* `port`: Port on which the WebMMBServer will listen
* `domain`: Internet domain name on which the server is running

Example

    {
        "mmb_exec_path": "/opt/bin/MMB",
        "mmb_parameters_path": "/opt/share/MMB/parameters.csv",
        "jobs_dir": "/tmp/webmmbsrv",
        "root_dir": "/opt/www/WebMMB",
        "port": 8000,
        "domain": "localhost"
    }
