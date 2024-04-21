# Slurm Task Manager - stama
A terminal userinterface for monitoring and managing slurm jobs.

# Content:

# Installation
1. Prerequisites: Rust compiler rustc and the rust packager manager cargo. They can be installed with 
```bash
curl https://sh.rustup.rs -sSf | sh
```
2. Install stama using the cargo package manager
```bash
cargo install stama
```
3. (Optional): To enable some functions, for example 'cd to working directory of selected job', add the following function to the config file of your shell, e.g. '($HOME)/.bashrc' for bash, or '($HOME)/.zshrc' for zsh:
```bash
stama() {
  temp_file="$(mktemp -t "stama.XXXXXXXXXX")"
  stama --output-file="$temp_file"
  output=$(cat -- "$temp_file")
  eval $output
  rm -f -- "$temp_file"
}
```


