# Menu Types:

- Job Overview: top level, showing a list of running jobs
    - full screen
- Job Actions: Job actions (kill, suspend, resume, etc.)
    - floating window
- Options: options for the user
    - floating window
- Job Allocation: code-remote like menu for job allocation
    - full screen
- Help: help for the user
    - floating window
- Confirmation: confirmation dialog
    - floating window
- Message: message dialog
    - floating window

# Job Overview
## Layout
                    JOB OVERVIEW
-- Job list: squeue -U u301533 --------------------------
| Job ID | Job Name | Status | Time | Partition | Nodes |
| 1      | job1     | Running| 1:00 | normal    | 1     |
*|2      | job2     | Queued | 0:00 | normal    | 1     |*
| 3      | job3     | Running| 0:30 | normal    | 1     |
(--------------------------------------------------------
-- *1. Job Details* - 2. Log ----------------------------
|                                                       |
(--------------------------------------------------------
      press 'Ctrl+C' or 'q' for exit, '?' for help

## Controls
- Down/Up (j/k): Next/Previous job
- Enter (l): Open job actions menu
- tab: Switch between sorting option
- r: reverse sorting order
- Left/Right: Switch focus between job details and log
- 1: Focus on job details
- 2: Focus on log
- a: Open allocation menu
- o: Open options menu
- /: Modify job list filter
- m: Minimize/Maximize top section
- n: Minimize/Maximize bottom section

## Color Codes
- Running: Green
- Queued: Yellow
- Suspended: Orange
- Completed: Gray

## Job details
Output of `scontrol show job <jobid>`

## log
If the job has an existing log file, show the tail of the log file.
If the log file does not exist, show "No log file found"
If no log file is specified, show "Job has no log file"


# Job actions menu
## Layout
-- Background ------------------------------------------
|                                                      |
|     - Job Actions ------------------------------     |
|     | 1. Kill job                              |     |
|     |*2. Open logfile*                         |     |
|     | 3. Open submission script                |     |
|     | 4. cd to working directory               |     |
|     | 5. ssh to node                           |     |
|     --------------------------------------------     |)
|                                                      |
(--------------------------------------------------------
      press 'Ctrl+C' or 'q' for exit, '?' for help

## Controls
- Down/Up (j/k): Next/Previous action
- Enter (l): Perform action
- Esc (h): Close menu
- 1-5: Perform corresponding action

## Actions
1. Kill job: `scancel <jobid>` (opens a popup menu for confirmation)
2. Open logfile: `$editor <logfile>`
3. Open submission script: `$editor <submission_script>`
4. cd to working directory: exit stm and returns `cd <working_directory>` 
5. ssh to node: exit stm and returns `ssh <batch_host>`

Note: If a wrapper script is used, the return string is executed in the shell

# Options
## Layout
-- Background ------------------------------------------
|                                                      |
|     - Options ------------------------------         |
|     | refresh interval:     1000 ms        |         |
|     |*show completed jobs:* yes            |         |
|     | external editor:      vim            |         |
|     | ...                                  |         |
|     ----------------------------------------         i
|                                                      |
(-------------------------------------------------------
      press 'Ctrl+C' or 'q' for exit, '?' for help

## Option Types
- Text: `option: value`
- Toggle: `option: [yes/no]`
- Integer: `option: value`

## Controls
- Down/Up (j/k): Next/Previous option
- Enter (i): Modify option
    - Toggle: Switch between yes/no
    - Text: Enter text edit
    - Integer: Enter integer edit
- Esc (h): Close menu


# Job Allocation
## Layout
                    JOG ALLOCATION
-*Presets:*--------| Settings: ----------------(<tab>)-
| preset1          | Preset Name:    preset2          |
|*preset2*         | Partition:      normal           |
| preset3          | Nodes:          1                |
|                  | CPUs per node:  1                |
|                  | Memory:         16G              |
|                  | Max. Time:      00:10:00         |
|                  | Other Options:                   |
(------------------------------------------------------
      press 'Ctrl+C' or 'q' for exit, '?' for help

## Controls
- Tab: Switch between presets and settings
- Right/Left (h/l): Switch between presets and settings
- Esc: Close menu
### Presets
- Down/Up (j/k): Next/Previous preset
- Enter: Allocate job with selected preset
### Settings
- Down/Up (j/k): Next/Previous setting
- Enter: Modify setting (enter edit mode)


# Help
## Layout
-- Background ------------------------------------------
|                                                      |
|     - Help -------------------------------------     |
|     | Job Overview (red)                       |     |
|     |   Down/Up (j/k): Next/Previous job       |     |
|     |   ...                                    |     |
|     | Job Actions (red)                        |     |
|     |   ...                                    |     |
|     | Job Allocation (red)                     |     |
|     |   Tab: Switch between presets and setting|     |
|     |  - Presets (orange)                      |     |
|     |   Down/Up (j/k): Next/Previous preset    |     |
|     |   ...                                    |     |
|     --------------------------------------------     |
|                                                     .|
--------------------------------------------------------.
      press 'Ctrl+C' or 'q' for exit, '?' for help

## Controls
- Down/Up (j/k): Scroll down/up
- Esc (h): Close menu

## When opening the help menu:
- The current opened menu should be positioned at the top
  or as close to the top as possible


# Confirmation
## Layout
-- Background ------------------------------------------
|                                                      |
|     --------------------------------------------    .|
|     | Some Dialog                              |     |
|     | more lines                               |     |
|     |            --------   --------           |     |
|     |            | Yes  |   |  No  |           |     |
|     |            --------   --------           |     |
|     --------------------------------------------     |
|                                                     .|
--------------------------------------------------------.
      press 'Ctrl+C' or 'q' for exit, '?' for Help

## Controls
- Left/Right (h/l): Switch between Yes/No
- Enter: Confirm selection
- y/n: Confirm selection
- Esc: Close menu (selects No)


# Message
## Layout
-- Background ------------------------------------------
|                                                      |
|     - Message ----------------('Esc' to close)--     |
|     | This is a message                        |     |
|     | with multiple lines                      |     |
|     |                                          |    .|
|     --------------------------------------------     |
|                                                     .|
--------------------------------------------------------.
      press 'Ctrl+C' or 'q' for exit, '?' for Help

## Controls
- Down/Up (j/k): Scroll down/up
- Esc: Close menu
