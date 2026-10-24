#!/usr/bin/env bash
# Written in [Amber](https://amber-lang.com/)
# version: 0.6.0-alpha
if [ -n "$ZSH_VERSION" ]; then
    EXEC_SHELL="zsh"
    IFS='.' read -A EXEC_SHELL_VERSION <<< "$ZSH_VERSION"
elif [ -n "$KSH_VERSION" ]; then
    EXEC_SHELL="ksh"
    __exec_shell_version="${.sh.version##*/}"
    IFS='.' read -a EXEC_SHELL_VERSION <<< "${__exec_shell_version%% *}"
else
    EXEC_SHELL="bash"
    EXEC_SHELL_VERSION=("${BASH_VERSINFO[0]}" "${BASH_VERSINFO[1]}" "${BASH_VERSINFO[2]}")
fi
# dir_exists(path: Text)
dir_exists__38_v0() {
    local path_5="${1}"
    [ -d "${path_5}" ]
    __status=$?
    ret_dir_exists38_v0="$(( __status == 0 ))"
    return 0
}

repo_name_3="Foundry-Worker"
dir_exists__38_v0 "/app/data/${repo_name_3}"
cloned_6="${ret_dir_exists38_v0}"
if [ "$(( ! cloned_6 ))" != 0 ]; then
    echo "Repo not cloned, cloning now..."
    cd /app/data/
    __status=$?
    git clone https://github.com/MADMAN-Modding/${repo_name_3}
    __status=$?
    echo "Repo cloned."
fi
cd /app/data/${repo_name_3}
__status=$?
if [ "${cloned_6}" != 0 ]; then
    echo "Repo already cloned, pulling latest..."
    git pull
    __status=$?
    echo "Latest pulled."
fi
echo "Copying .env..."
cp .env /${repo_name_3}
__status=$?
echo "Copied .env."
echo "Running bot, ensure .env is set correctly"
cargo run --release
__status=$?
