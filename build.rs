// use std::process::Command;

// fn main() {
//     // If the build succeeds, print a message
//     println!("cargo:rerun-if-changed=web"); // Tell Cargo to rerun if the `web` directory changes

//     // Define the command to run `pnpm build` in the `./web` directory
//     let status = Command::new("pnpm")
//         .arg("build")
//         .current_dir("./web")
//         .status()
//         .expect("Failed to execute pnpm build");

//     // Check if the build was successful
//     if !status.success() {
//         panic!("Frontend build failed. Please check the logs for details.");
//     }

//     // Continue with the Rust project build
//     // Additional Rust build configuration (if any) goes here
// }

fn main() {
    
}