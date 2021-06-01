# WebMMB and WebMMBServer

WebMMB is a web-based service for the [MacroMoleculeBuilder (MMB)](https://github.com/samuelflores/MMB) structural biology tool.

WebMMBServer is the backend of the service. WebMMBServer manages user sessions and controls MMB jobs. In order to function, WebmMMBServer requires MMB to be installed on the server where it runs.

## Installation
WebMMBServer is based on [Rocket](https://rocket.rs/) library which requires the latest Rust nightly toolchain. Follow the instructions at [rustup.rs](https://rustup.rs/) to set up Rust development toolchain. Install the `nightly` toolchain by issuing

    rustup toolchain install nightly

command in the terminal.

`cd` into the directory with WebMMBServer project sources and set the `nightly` toolchain as default for the project with the following command:

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
* `domain`: Internet domain name on which the server will run. This must be set correctly for the session cookies to work properly. Set this to `localhost` if you are running a local WEbMMBServer instance that is not accessible over the Internet
* `require_https`: Require HTTPS connection. If you are not using HTTPS, this must be set to `false`
* `use_pbs_offloading`: Run jobs through PBS job management system. This requires a working PBS installation on the server.

Example

    {
        "mmb_exec_path": "/opt/bin/MMB",
        "mmb_parameters_path": "/opt/share/MMB/parameters.csv",
        "jobs_dir": "/tmp/webmmbsrv",
        "root_dir": "/opt/www/WebMMB",
        "port": 8000,
        "domain": "localhost",
        "require_https", true,
        "use_pbs_offloading": false,
    }
