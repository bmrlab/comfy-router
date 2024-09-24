# Comfy Router

## Features

Comfy Router is a unified entry point for ComfyUI node management and workflow execution, with the following features:

- **Simplified workflow execution**. Workflows can be executed without inputting JSON files, directly callable through predefined APIs for basic SD15, SDXL, and Flux workflows.
- **Web-based node management and simple load balancing**. Comfy Router provides an admin page for adding and removing nodes. It also automatically selects appropriate nodes when executing workflows.
- **Automatic file download and caching**. For models, images, and other files in workflows, URLs can be passed in. Comfy Router manages the downloading and storage of these URLs, avoiding repeated downloads and excessive caching.
- **Basic authentication**. All APIs (except preview and health check) have Basic Authentication, including the admin page.

### Workflow Execution

Comfy Router has built-in definitions for 3 basic workflows, which can be triggered directly through `POST /workflow`.  
For specific API parameters, refer to the project's OpenAPI documentation (`/doc`).  
During workflow execution, Comfy Router and nodes communicate via WebSocket, distinguishing information through `prompt_id` and `client_id`, and updating task status in real-time. The `/workflow/:id` API can be used to query task status and view the generation process.

### Node Management and Load Balancing

By logging into the admin page (`/admin`) with a username and password, nodes can be easily added and removed. Nodes have 3 states: `Idle`, `Busy` and `Offline`. All nodes undergo health checks, and when a health check fails, the node automatically switches to `Offline` status. `Idle` and `Busy` correspond to the free and busy (currently executing workflow) node states respectively.  
When a workflow trigger request is received, Comfy Router immediately returns the task id and asynchronously starts task execution in the background (based on tokio::spawn).  
When execution, files passed in via URL in the workflow are downloaded firstly. Then, Comfy Router automatically selects an `Idle` node to begin workflow execution and set its state to `Busy`. After the workflow completes, the node automatically switches to `Idle`.

### File Download and Caching

Files passed in via URL in the workflow are automatically downloaded and cached before workflow execution. The cache has a size limit (set through environment variables), and when the cache size exceeds the limit, the least recently used files will be deleted.  
The cache is first downloaded to a public cache folder, then symlinked for ComfyUI to use.

### Authentication

Except for the `/preview/:id` and `/health_check` APIs, all requests require Basic Authentication. The username and password for Basic Authentication can also be set through environment variables.

## Development

> Recommended versions: Rust 1.80 and above, node 20.9 and above, pnpm 8.10 and above

### API Service

Install dependencies: `cargo install`  
Start: `cargo run`  

### Admin page (Frontend)

Switch to web directory: `cd web`  
Install dependencies: `pnpm install`  
Start: `pnpm dev`  

## Deployment

Comfy Router can be deployed independently or started in the same container as ComfyUI

Comfy Router needs to share disk with all ComfyUI nodes (for downloads):

- The following ComfyUI paths need to be readable and writable by Comfy Router:
  - `%COMFY_UI_ROOT%/models`
  - `%COMFY_UI_ROOT%/input`
- The following Comfy Router path needs to be readable by ComfyUI:
  - `COMFY_ROUTER__DOWNLOAD__CACHE_DIR` (see environment variables)

After service startup, all nodes need to be manually added on the admin page, ensuring network connectivity between Comfy Router and ComfyUI (can be through public or private network)

### Building

It's recommended to use the version from [release](https://github.com/bmrlab/comfy-router/releases).  
If manual building or separate image building is needed, refer to the GitHub Action definition and rewrite the Dockerfile.  
The main steps are:

- **Build the frontend project**. Switch to the `web` directory, execute `pnpm build` to complete the build, the output should be in the `web/dist` directory. Ensure `web/dist` is still available in the next step.
- **Build the API service**. Simply execute `cargo build --release`. Note, if you need to build executable files on a Mac for use on production machines, there are two methods:
  - Use `cargo build --release --target x86_64-unknown-linux-gnu` directly. However, dependency issues may occur, which can be complicated.
  - Use [cross](https://github.com/cross-rs/cross), simply `cross build --release`. Because it's built in a container, there are no dependency issues, but it's indeed slower.

### Environment Variables

**COMFY_ROUTER__HOST**  
Application HOST, default is 0.0.0.0

**COMFY_ROUTER__PORT**  
Application port, default is 8080

**COMFY_ROUTER__USERNAME**  
Basic Authentication username, default is admin

**COMFY_ROUTER__PASSWORD**  
Basic Authentication password, default is admin

**COMFY_ROUTER__HISTORY_LIMIT**  
Maximum cache size for workflow history records (old results will be discarded when reached), default is 50

**COMFY_ROUTER__PENDING_LIMIT**  
Maximum waiting length for workflows (new execution requests will receive a 429 Too Many Requests error when reached), default is 25

**COMFY_ROUTER__ENV**  
Application running environment, currently unused, default is dev

**COMFY_ROUTER__DOWNLOAD__CACHE_DIR**  
Download cache directory, this directory should be readable by ComfyUI, default is /tmp/cache

**COMFY_ROUTER__DOWNLOAD__ROOT_DIR**  
ComfyUI directory (note this is the ComfyUI directory readable by Comfy Router, it may be different if not deployed in the same container), default is /tmp/model

**COMFY_ROUTER__DOWNLOAD__RECORD_PATH**  
Path for download cache records, default is /tmp/record.json

**COMFY_ROUTER__DOWNLOAD__MAX_CACHE_BYTES**  
Maximum size of download cache directory in Bytes, default is 1024 * 1024 * 1024 * 64, i.e., 64 GB
