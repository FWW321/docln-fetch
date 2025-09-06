#!/usr/bin/env nu

# 发布脚本 - 用于创建新版本并触发GitHub Actions构建
# 使用方法: nu scripts/release.nu [版本号]

use std

# 颜色定义
const RED = "\e[31m"
const GREEN = "\e[32m"
const YELLOW = "\e[33m"
const BLUE = "\e[34m"
const NC = "\e[0m"  # No Color

# 检查是否在git仓库中
def check_git_repo [] {
    try {
        git rev-parse --is-inside-work-tree | str trim
        true
    } catch {
        print $"($RED)错误: 当前目录不是Git仓库($NC)"
        false
    }
}

# 检查是否有未提交的更改
def check_uncommitted_changes [] {
    try {
        let status = git status --porcelain
        if ($status | is-empty) {
            true
        } else {
            print $"($RED)错误: 有未提交的更改，请先提交或暂存($NC)"
            print $"未提交的更改:"
            $status | each { |line| print $"  $line" }
            false
        }
    } catch {
        print $"($RED)错误: 无法检查git状态($NC)"
        false
    }
}

# 获取当前版本
def get_current_version [] {
    try {
        open Cargo.toml | get package.version
    } catch {
        print $"($RED)错误: 无法读取Cargo.toml中的版本号($NC)"
        exit 1
    }
}

# 验证版本号格式
def validate_version [version: string] {
    ($version | parse "{major}.{minor}.{patch}") | is-not-empty
}

# 获取新版本号
def get_new_version [current: string, input?: string] {
    if ($input != "" and $input != null) {
        let version = $input | str trim
        if (validate_version $version) {
            $version
        } else {
            print $"($RED)错误: 版本号格式无效，应为 x.y.z($NC)"
            exit 1
        }
    } else {
        # 自动增加补丁版本
        let parts = ($current | split row ".")
        let major = $parts.0
        let minor = $parts.1
        let patch = (($parts.2 | into int) + 1 | into string)
        $"($major).($minor).($patch)"
    }
}

# 更新Cargo.toml中的版本号
def update_cargo_version [new_version: string] {
    try {
        let content = open Cargo.toml
        let updated = $content | upsert package.version $new_version
        $updated | save Cargo.toml --force
        print $"($GREEN)已更新Cargo.toml版本号为: ($new_version)($NC)"
    } catch {
        print $"($RED)错误: 无法更新Cargo.toml($NC)"
        exit 1
    }
}

# 提交版本更新
def commit_version_update [new_version: string] {
    try {
        git add Cargo.toml
        git commit -m ($"chore: bump version to ($new_version)")
        print $"($GREEN)已提交版本更新($NC)"
    } catch {
        print $"($RED)错误: 无法提交更改($NC)"
        exit 1
    }
}

# 创建标签
def create_tag [new_version: string] {
    try {
        # 检查标签是否已存在
        let tag_exists = (git tag -l $"v($new_version)" | str trim | is-not-empty)
        if $tag_exists {
            print $"($RED)错误: 标签 v($new_version) 已存在($NC)"
            exit 1
        }
        
        git tag -a ($"v($new_version)") -m ($"Release version ($new_version)")
        print $"($GREEN)已创建标签: v($new_version)($NC)"
    } catch {
        print $"($RED)错误: 无法创建标签($NC)"
        exit 1
    }
}

# 推送到远程仓库
def push_to_remote [new_version: string] {
    try {
        print $"($YELLOW)推送到远程仓库...($NC)"
        let current_branch = (git branch --show-current | str trim)
        git push origin $current_branch
        git push origin $"v($new_version)"
        print $"($GREEN)推送成功！($NC)"
    } catch {
        print $"($RED)错误: 无法推送到远程仓库($NC)"
        exit 1
    }
}

# 主函数
# 主函数将在其他Nu代码之后自动运行
def main [version?: string] {
    print $"($BLUE)docln-fetch - 小说爬取工具发布脚本($NC)"
    print "========================================"

    # 检查环境
    if not (check_git_repo) {
        exit 1
    }

    if not (check_uncommitted_changes) {
        exit 1
    }

    # 获取当前版本
    let current_version = get_current_version
    print $"($YELLOW)当前版本: ($current_version)($NC)"

    # 获取新版本号
    let input_version = if ($version != null) { $version } else { "" }
    let has_explicit_version = ($input_version != "" and $input_version != null)
    let new_version = get_new_version $current_version $input_version
    
    if (not $has_explicit_version) {
        print $"($YELLOW)自动增加版本号: ($current_version) → ($new_version)($NC)"
    } else {
        print $"($YELLOW)指定版本号: ($new_version)($NC)"
    }

    # 确认操作
    print $"是否继续发布版本 ($new_version)? \(y/N\): "
    let confirm = (input)
    
    if ($confirm | str downcase) != "y" {
        print "取消发布"
        exit 0
    }

    # 执行发布流程
    if ($new_version != $current_version) {
        update_cargo_version $new_version
        commit_version_update $new_version
    } else {
        print $"($YELLOW)版本号未改变，跳过更新 Cargo.toml($NC)"
    }
    create_tag $new_version
    push_to_remote $new_version

    print $"($GREEN)发布成功！GitHub Actions 将自动构建并创建 Release($NC)"
}