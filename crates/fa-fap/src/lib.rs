pub mod error;
pub mod invoke;
pub mod manifest;
pub mod package;
pub mod parser;
pub mod platform;
pub mod process;

pub use error::FapError;
pub use invoke::{InvokeError, SpecialVars, render_invoke};
pub use manifest::{
    Action, CapabilityDomain, InvokeConfig, Lifecycle, Manifest, OutputConfig, PackageMode,
    ParamDef, SignatureInfo,
};
pub use package::{PackageInfo, install_package, inspect_package, list_packages, uninstall_package};
pub use parser::{ParserError, parse_output};
pub use platform::detect_platform;
pub use process::{ProcessError, ProcessResult, execute_process};
