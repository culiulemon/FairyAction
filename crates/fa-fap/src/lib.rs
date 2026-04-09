pub mod error;
pub mod invoke;
pub mod manifest;
pub mod package;
pub mod parser;
pub mod platform;
pub mod process;
pub mod process_pool;
pub mod pack;
pub mod permission;
pub mod sign;
pub mod version;

pub use error::FapError;
pub use version::{FapVersion, VersionError};
pub use permission::validate_permissions;
pub use pack::{PackError, pack_fap};
pub use invoke::{InvokeError, SpecialVars, render_invoke};
pub use manifest::{
    Action, CapabilityDomain, InvokeConfig, Lifecycle, Manifest, OutputConfig, PackageMode,
    ParamDef, SignatureInfo,
};
pub use package::{InstallResult, PackageInfo, VersionChange, install_package, inspect_package, list_packages, uninstall_package};
pub use parser::{ParserError, parse_output};
pub use platform::detect_platform;
pub use process::{ProcessError, ProcessResult, execute_process};
pub use process_pool::{CallResult, ProcessPool};
pub use sign::{Keypair, SignError, compute_digest, generate_keypair, sign_package, verify_package, write_keypair};
