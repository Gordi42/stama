# Slurm Task Manager - stama
A terminal user interface for monitoring and managing slurm jobs.
![20240421_19h50m42s_grim](https://github.com/Gordi42/stama/assets/118457787/2bf9098f-9643-43a2-9a6f-217403acd5cc)

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
![20240421_19h54m40s_grim](https://github.com/Gordi42/stama/assets/118457787/da30db31-c71f-4ead-952d-58b9e6433de3)
3. **User settings:** The user settings menu can be accessed by pressing 'o' inside stama. The available options can be modified by selecting them and pressing 'enter'.
![20240421_19h56m32s_grim](https://github.com/Gordi42/stama/assets/118457787/fa882ab6-e40b-4712-827f-fa6c879aa0bc)
4. **Job actions:** The job actions menu of the selected job can be accessed by pressing 'enter' inside stama. Available job actions are:
    - Cancel job (only with permission)
    - Open job output file in editor (default: vim, can be changed in user settings)
    - Open job submission script in editor (default: vim, can be changed in user settings)
    - cd to working directory of selected job (only in stama_wrapper)
    - ssh to node of selected job (only in stama_wrapper)
![20240421_19h53m33s_grim](https://github.com/Gordi42/stama/assets/118457787/f69d16b3-e010-40a7-8b68-0060fe96e246)
5. **Job allocation:** The job allocation menu can be accessed by pressing 'a' inside stama. The job allocation menu shows a list of saved salloc commands New presets can be created by navigating to the 'create new' entry.
![20240421_19h48m15s_grim](https://github.com/Gordi42/stama/assets/118457787/23bb3bc0-1746-46e3-ba5f-2d7ab998ccc0)
6. **Change squeue command:** Press '/' or click on the squeue command with the mouse to change the squeue command, 'squeue' without any additional arguments will show all running jobs from all users.

**For more infos see:** [notes.md](notes.md)


# Author
Silvano Rosenau
