# Slurm Task Manager - stama
A terminal userinterface for monitoring and managing slurm jobs.

# Content:
- [Installation](#installation)
- [Usage](#usage)

# Installation
1. Prerequisites: Rust compiler rustc and the rust packager manager cargo. They can be installed with 
```bash
curl https://sh.rustup.rs -sSf | sh
```
2. Install stama using the cargo package manager
```bash
cargo install stama
```
The program 'stama' should now be available in your terminal.
3. (Optional): To enable some functions, for example 'cd to working directory of selected job', add the following function to the config file of your shell, e.g. '($HOME)/.bashrc' for bash, or '($HOME)/.zshrc' for zsh:
```bash
stama_wrapper() {
  temp_file="$(mktemp -t "stama.XXXXXXXXXX")"
  stama --output-file="$temp_file"
  output=$(cat -- "$temp_file")
  # Check if the output is "cd /some/path/"
  if [[ "$output" == "cd "* ]]; then
    local directory="${output#cd }"  # Extract the directory path
    cd "$directory"
  # Check if the output is "ssh <some_address>"
  elif [[ "$output" == "ssh "* ]]; then
    local node="${output#ssh }"  # Use the output as the SSH command
    ssh -Y "$node"
  else
    echo "Unknown command in stama output: $output"
  fi
  rm -f -- "$temp_file"
}
```
After restarting your terminal or sourcing the config file, you can use the function 'stama_wrapper' to execute the commands output by stama.

# Usage
1. **Starting stama:** Stama can be started by executing 'stama' or 'stama_wrapper' in your terminal.
2. **All keybindings:** The keybindings info can be accessed by pressing '?' inside stama.
3. **User settings:** The user settings menu can be accessed by pressing 'o' inside stama. The available options can be modified by selecting them and pressing 'enter'.
4. **Job actions:** The job actions menu of the selected job can be accessed by pressing 'enter' inside stama. Available job actions are:
    - Cancel job (only with permission)
    - Open job output file in editor (default: vim, can be changed in user settings)
    - Open job submission script in editor (default: vim, can be changed in user settings)
    - cd to working directory of selected job (only in stama_wrapper)
    - ssh to node of selected job (only in stama_wrapper)
5. **Job allocation:** The job allocation menu can be accessed by pressing 'a' inside stama. The job allocation menu shows a list of saved salloc commands New presets can be created by navigating to the 'create new' entry.

**For more infos see notes.md**


# Author
Silvano Rosenau
