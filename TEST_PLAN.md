# SideX 项目测试用例开发计划

## 一、项目概述

SideX 是一个基于 Tauri + Rust 构建的现代化跨平台代码编辑器。项目采用模块化架构，包含前端（TypeScript/Web）和后端（Rust）两部分。

### 1.1 技术栈

- **前端**: TypeScript, Monaco Editor, VSCode 架构
- **后端**: Rust, Tauri v2
- **构建工具**: Vite, Cargo
- **测试框架**: 
  - Rust: 内置 `#[test]`, `#[cfg(test)]`
  - TypeScript: Mocha, Chai

### 1.2 核心模块

| 模块类别 | 主要功能 | 技术栈 |
|---------|---------|--------|
| 文件系统 | 文件读写、目录操作、元数据 | Rust |
| Git 集成 | 版本控制所有操作 | Rust + TypeScript |
| 终端 | PTY 管理、Shell 集成 | Rust |
| 编辑器 | 文本处理、语法高亮 | TypeScript + Rust |
| LSP | 语言服务器协议通信 | Rust |
| 调试 | DAP 协议、调试适配器 | Rust |
| 扩展 | VSIX 安装、扩展管理 | Rust |
| 搜索 | 文件搜索、全文搜索 | Rust |

---

## 二、测试用例优先级

### 2.1 高优先级（核心功能）

这些模块直接影响用户体验，必须优先覆盖：

1. **Git 操作** - 版本控制核心功能
2. **文件读写** - 最基本的编辑功能
3. **终端** - 开发者常用工具
4. **编辑器核心** - 文本编辑基础

### 2.2 中优先级（重要功能）

这些功能对完整 IDE 体验很重要：

5. **LSP 通信** - 代码智能提示
6. **搜索功能** - 快速定位代码
7. **调试功能** - 问题排查工具
8. **扩展管理** - 生态系统的核心

### 2.3 低优先级（辅助功能）

这些功能可以在核心功能稳定后补充：

9. **设置管理** - 用户配置
10. **主题管理** - 界面美化
11. **远程连接** - SSH/Docker

---

## 三、测试用例详细规划

### 3.1 Git 操作测试 (`git.rs`)

#### 测试文件位置
- Rust: `src-tauri/src/commands/git.rs` 
- TypeScript: `src/vs/workbench/contrib/scm/browser/git.contribution.ts`

#### 已有的测试
- ✅ `decodeGitGraphQuery` - Base64 解码
- ✅ `urlencoding` - URL 编解码

#### 需要补充的测试

##### 文件: `tests/git_operations_test.rs`

```rust
#[cfg(test)]
mod git_operations_tests {
    use super::*;

    // 3.1.1 Git 状态测试
    #[test]
    fn test_git_status_clean() {
        // 测试空仓库状态
        // 预期: 无变更文件列表
    }
    
    #[test]
    fn test_git_status_with_changes() {
        // 测试有变更的状态
        // 预期: 正确识别已修改、新增、删除的文件
    }
    
    #[test]
    fn test_git_status_untracked_files() {
        // 测试未跟踪文件
        // 预期: 正确识别新文件
    }
    
    #[test]
    fn test_git_status_ignored_files() {
        // 测试 .gitignore 文件
        // 预期: 不显示忽略的文件
    }

    // 3.1.2 Git Diff 测试
    #[test]
    fn test_git_diff_staged() {
        // 测试暂存区的 diff
    }
    
    #[test]
    fn test_git_diff_unstaged() {
        // 测试未暂存的 diff
    }
    
    #[test]
    fn test_git_diff_binary_file() {
        // 测试二进制文件
        // 预期: 显示为二进制文件
    }
    
    #[test]
    fn test_git_diff_renamed_file() {
        // 测试重命名文件
    }
    
    #[test]
    fn test_git_diff_large_file() {
        // 测试大文件的 diff
        // 预期: 正确分页或截断
    }

    // 3.1.3 Git 提交测试
    #[test]
    fn test_git_commit_single_file() {
        // 测试单文件提交
    }
    
    #[test]
    fn test_git_commit_multiple_files() {
        // 测试多文件提交
    }
    
    #[test]
    fn test_git_commit_empty_message() {
        // 测试空提交信息
        // 预期: 错误处理
    }
    
    #[test]
    fn test_git_commit_no_staged() {
        // 测试无暂存文件时提交
        // 预期: 错误处理
    }
    
    #[test]
    fn test_git_commit_with_signoff() {
        // 测试带 signoff 的提交
    }

    // 3.1.4 Git 分支测试
    #[test]
    fn test_git_branch_list() {
        // 测试分支列表
    }
    
    #[test]
    fn test_git_branch_create() {
        // 测试创建分支
    }
    
    #[test]
    fn test_git_branch_delete() {
        // 测试删除分支
    }
    
    #[test]
    fn test_git_branch_delete_current() {
        // 测试删除当前分支
        // 预期: 错误处理
    }
    
    #[test]
    fn test_git_branch_rename() {
        // 测试重命名分支
    }
    
    #[test]
    fn test_git_checkout_file() {
        // 测试检出特定文件
    }
    
    #[test]
    fn test_git_checkout_branch() {
        // 测试切换分支
    }

    // 3.1.5 Git 日志测试
    #[test]
    fn test_git_log_basic() {
        // 测试基本日志
    }
    
    #[test]
    fn test_git_log_with_diff() {
        // 测试带 diff 的日志
    }
    
    #[test]
    fn test_git_log_limit() {
        // 测试日志数量限制
    }
    
    #[test]
    fn test_git_log_author_filter() {
        // 测试按作者过滤
    }
    
    #[test]
    fn test_git_log_date_filter() {
        // 测试按日期过滤
    }
    
    #[test]
    fn test_git_log_graph() {
        // 测试带图形化的日志
    }

    // 3.1.6 Git 远程操作测试
    #[test]
    fn test_git_push_new_branch() {
        // 测试推送新分支
    }
    
    #[test]
    fn test_git_push_existing_branch() {
        // 测试推送已有分支
    }
    
    #[test]
    fn test_git_push_force() {
        // 测试强制推送
    }
    
    #[test]
    fn test_git_pull_no_conflicts() {
        // 测试无冲突的拉取
    }
    
    #[test]
    fn test_git_fetch() {
        // 测试获取远程更新
    }

    // 3.1.7 Git Blame 测试
    #[test]
    fn test_git_blame_basic() {
        // 测试基本的 blame
    }
    
    #[test]
    fn test_git_blame_with_options() {
        // 测试带选项的 blame
    }

    // 3.1.8 Git Stash 测试
    #[test]
    fn test_git_stash_save() {
        // 测试储藏
    }
    
    #[test]
    fn test_git_stash_list() {
        // 测试储藏列表
    }
    
    #[test]
    fn test_git_stash_pop() {
        // 测试恢复储藏
    }
    
    #[test]
    fn test_git_stash_drop() {
        // 测试删除储藏
    }

    // 3.1.9 Git 合并测试
    #[test]
    fn test_git_merge_fast_forward() {
        // 测试快进合并
    }
    
    #[test]
    fn test_git_merge_conflict() {
        // 测试合并冲突
        // 预期: 正确识别冲突
    }
    
    #[test]
    fn test_git_merge_no_ff() {
        // 测试禁用快进合并
    }

    // 3.1.10 Git 仓库测试
    #[test]
    fn test_git_clone() {
        // 测试克隆仓库
    }
    
    #[test]
    fn test_git_clone_with_depth() {
        // 测试浅克隆
    }
    
    #[test]
    fn test_git_is_repo() {
        // 测试判断仓库
    }
    
    #[test]
    fn test_git_find_repos() {
        // 测试查找仓库
    }

    // 3.1.11 Git 标签测试
    #[test]
    fn test_git_tag_list() {
        // 测试标签列表
    }
    
    #[test]
    fn test_git_tag_create() {
        // 测试创建标签
    }
    
    #[test]
    fn test_git_tag_delete() {
        // 测试删除标签
    }
    
    #[test]
    fn test_git_tag_annotated() {
        // 测试带注释的标签
    }

    // 3.1.12 错误处理测试
    #[test]
    fn test_git_invalid_path() {
        // 测试无效路径
    }
    
    #[test]
    fn test_git_permission_denied() {
        // 测试权限拒绝
    }
    
    #[test]
    fn test_git_not_a_repo() {
        // 测试非仓库目录
    }
    
    #[test]
    fn test_git_merge_conflict_marker() {
        // 测试冲突标记文件
    }
}
```

##### 文件: `tests/git_graph_provider_test.ts`

```typescript
describe('GitGraphContentProvider', function () {
    describe('provideTextContent', function () {
        it('should resolve valid git-graph URI')
        it('should handle missing file')
        it('should handle invalid commit')
        it('should handle non-existent repo')
        it('should set correct language mode')
        it('should handle binary files')
    });
    
    describe('decodeGitGraphQuery', function () {
        // 已有的测试用例...
        it('should handle very long file paths')
        it('should handle unicode commit messages')
        it('should handle special characters in repo path')
    });
});
```

---

### 3.2 文件系统测试 (`fs.rs`)

#### 测试文件位置
- Rust: `src-tauri/src/commands/fs.rs`

#### 需要补充的测试

##### 文件: `tests/fs_operations_test.rs`

```rust
#[cfg(test)]
mod fs_operations_tests {
    // 3.2.1 读取文件测试
    #[test]
    fn test_read_file_basic() {
        // 测试基本文件读取
    }
    
    #[test]
    fn test_read_file_empty() {
        // 测试空文件
    }
    
    #[test]
    fn test_read_file_large() {
        // 测试大文件 (>10MB)
    }
    
    #[test]
    fn test_read_file_binary() {
        // 测试二进制文件
    }
    
    #[test]
    fn test_read_file_utf8() {
        // 测试 UTF-8 编码
    }
    
    #[test]
    fn test_read_file_utf16() {
        // 测试 UTF-16 编码
    }
    
    #[test]
    fn test_read_file_with_spaces() {
        // 测试带空格的路径
    }
    
    #[test]
    fn test_read_file_unicode() {
        // 测试 Unicode 文件名
    }
    
    #[test]
    fn test_read_nonexistent() {
        // 测试读取不存在的文件
        // 预期: 错误处理
    }

    // 3.2.2 写入文件测试
    #[test]
    fn test_write_file_basic() {
        // 测试基本文件写入
    }
    
    #[test]
    fn test_write_file_overwrite() {
        // 测试覆盖文件
    }
    
    #[test]
    fn test_write_file_create_parent() {
        // 测试自动创建父目录
    }
    
    #[test]
    fn test_write_file_permission_denied() {
        // 测试权限拒绝
    }
    
    #[test]
    fn test_write_file_binary() {
        // 测试写入二进制
    }
    
    #[test]
    fn test_write_file_concurrent() {
        // 测试并发写入
    }

    // 3.2.3 目录操作测试
    #[test]
    fn test_read_dir_basic() {
        // 测试基本目录读取
    }
    
    #[test]
    fn test_read_dir_empty() {
        // 测试空目录
    }
    
    #[test]
    fn test_read_dir_recursive() {
        // 测试递归列出目录
    }
    
    #[test]
    fn test_mkdir_basic() {
        // 测试创建目录
    }
    
    #[test]
    fn test_mkdir_nested() {
        // 测试创建嵌套目录
    }
    
    #[test]
    fn test_mkdir_already_exists() {
        // 测试目录已存在
    }
    
    #[test]
    fn test_remove_file() {
        // 测试删除文件
    }
    
    #[test]
    fn test_remove_dir() {
        // 测试删除目录
    }
    
    #[test]
    fn test_remove_nonexistent() {
        // 测试删除不存在的文件
    }
    
    #[test]
    fn test_rename_file() {
        // 测试重命名文件
    }
    
    #[test]
    fn test_rename_dir() {
        // 测试重命名目录
    }
    
    #[test]
    fn test_rename_to_existing() {
        // 测试重命名为已存在的文件
    }

    // 3.2.4 文件元数据测试
    #[test]
    fn test_stat_file() {
        // 测试文件元数据
    }
    
    #[test]
    fn test_stat_directory() {
        // 测试目录元数据
    }
    
    #[test]
    fn test_stat_symlink() {
        // 测试符号链接
    }
    
    #[test]
    fn test_exists_true() {
        // 测试文件存在
    }
    
    #[test]
    fn test_exists_false() {
        // 测试文件不存在
    }

    // 3.2.5 路径处理测试
    #[test]
    fn test_path_normalization() {
        // 测试路径规范化
    }
    
    #[test]
    fn test_path_traversal() {
        // 测试路径遍历攻击防护
    }
    
    #[test]
    fn test_absolute_vs_relative() {
        // 测试绝对路径和相对路径
    }
    
    #[test]
    fn test_path_with_special_chars() {
        // 测试特殊字符路径
    }

    // 3.2.6 字节流测试
    #[test]
    fn test_read_file_bytes() {
        // 测试字节流读取
    }
    
    #[test]
    fn test_write_file_bytes() {
        // 测试字节流写入
    }
}
```

---

### 3.3 终端功能测试 (`terminal.rs`, `process.rs`)

#### 测试文件位置
- Rust: `src-tauri/src/commands/terminal.rs`, `process.rs`

#### 需要补充的测试

##### 文件: `tests/terminal_test.rs`

```rust
#[cfg(test)]
mod terminal_tests {
    // 3.3.1 PTY 生命周期测试
    #[test]
    fn test_terminal_spawn() {
        // 测试启动终端
    }
    
    #[test]
    fn test_terminal_multiple() {
        // 测试多终端
    }
    
    #[test]
    fn test_terminal_write() {
        // 测试写入终端
    }
    
    #[test]
    fn test_terminal_read() {
        // 测试读取终端输出
    }
    
    #[test]
    fn test_terminal_resize() {
        // 测试调整大小
    }
    
    #[test]
    fn test_terminal_kill() {
        // 测试终止终端
    }
    
    #[test]
    fn test_terminal_pid() {
        // 测试获取进程 ID
    }

    // 3.3.2 Shell 检测测试
    #[test]
    fn test_get_default_shell() {
        // 测试获取默认 shell
    }
    
    #[test]
    fn test_get_available_shells() {
        // 测试获取可用 shell 列表
    }
    
    #[test]
    fn test_shell_detection_linux() {
        // Linux: 检测 bash/zsh/fish
    }
    
    #[test]
    fn test_shell_detection_windows() {
        // Windows: 检测 PowerShell/CMD
    }
    
    #[test]
    fn test_shell_detection_macos() {
        // macOS: 检测 zsh/bash
    }

    // 3.3.3 进程执行测试
    #[test]
    fn test_exec_simple_command() {
        // 测试执行简单命令
    }
    
    #[test]
    fn test_exec_with_args() {
        // 测试带参数的命令
    }
    
    #[test]
    fn test_exec_with_cwd() {
        // 测试指定工作目录
    }
    
    #[test]
    fn test_exec_with_env() {
        // 测试设置环境变量
    }
    
    #[test]
    fn test_exec_timeout() {
        // 测试命令超时
    }
    
    #[test]
    fn test_exec_nonexistent() {
        // 测试执行不存在的命令
    }
    
    #[test]
    fn test_exec_exit_code() {
        // 测试命令退出码
    }

    // 3.3.4 缓冲区测试
    #[test]
    fn test_terminal_buffer_small() {
        // 测试小缓冲区
    }
    
    #[test]
    fn test_terminal_buffer_large() {
        // 测试大缓冲区 (>1MB)
    }
    
    #[test]
    fn test_terminal_buffer_clear() {
        // 测试清空缓冲区
    }

    // 3.3.5 ANSI 解析测试
    #[test]
    fn test_ansi_colors() {
        // 测试 ANSI 颜色
    }
    
    #[test]
    fn test_ansi_cursor_movement() {
        // 测试光标移动
    }
    
    #[test]
    fn test_ansi_clear_screen() {
        // 测试清屏
    }
    
    #[test]
    fn test_ansi_bell() {
        // 测试响铃
    }

    // 3.3.6 信号处理测试 (Unix)
    #[test]
    fn test_terminal_signal_sigint() {
        // 测试 SIGINT 信号
    }
    
    #[test]
    fn test_terminal_signal_sigterm() {
        // 测试 SIGTERM 信号
    }
    
    #[test]
    fn test_terminal_signal_sighup() {
        // 测试 SIGHUP 信号
    }

    // 3.3.7 错误处理测试
    #[test]
    fn test_terminal_closed() {
        // 测试终端已关闭
    }
    
    #[test]
    fn test_terminal_write_after_close() {
        // 测试关闭后写入
    }
    
    #[test]
    fn test_terminal_permission_denied() {
        // 测试权限拒绝
    }
}
```

---

### 3.4 文本处理测试 (`text.rs`)

#### 测试文件位置
- Rust: `src-tauri/src/commands/text.rs`
- Crate: `crates/sidex-text/`

##### 文件: `tests/text_processing_test.rs`

```rust
#[cfg(test)]
mod text_processing_tests {
    // 3.4.1 行数统计测试
    #[test]
    fn test_count_lines_empty() {
        // 测试空文件
    }
    
    #[test]
    fn test_count_lines_single() {
        // 测试单行
    }
    
    #[test]
    fn test_count_lines_multiple() {
        // 测试多行
    }
    
    #[test]
    fn test_count_lines_with_trailing_newline() {
        // 测试末尾换行符
    }
    
    #[test]
    fn test_count_lines_without_newline() {
        // 测试无末尾换行符
    }

    // 3.4.2 行尾处理测试
    #[test]
    fn test_normalize_line_endings_lf() {
        // 测试 LF 规范化
    }
    
    #[test]
    fn test_normalize_line_endings_crlf() {
        // 测试 CRLF 规范化
    }
    
    #[test]
    fn test_normalize_line_endings_mixed() {
        // 测试混合换行符
    }
    
    #[test]
    fn test_to_crlf() {
        // 测试转换为 CRLF
    }
    
    #[test]
    fn test_trim_trailing_whitespace() {
        // 测试去除尾随空白
    }

    // 3.4.3 单词边界测试
    #[test]
    fn test_word_boundaries_english() {
        // 测试英文单词边界
    }
    
    #[test]
    fn test_word_boundaries_chinese() {
        // 测试中文分词
    }
    
    #[test]
    fn test_word_boundaries_mixed() {
        // 测试中英混合
    }
    
    #[test]
    fn test_word_boundaries_punctuation() {
        // 测试标点符号
    }

    // 3.4.4 文件摘要测试
    #[test]
    fn test_file_summary_basic() {
        // 测试基本摘要
    }
    
    #[test]
    fn test_file_summary_binary() {
        // 测试二进制文件摘要
    }
    
    #[test]
    fn test_file_summary_empty() {
        // 测试空文件摘要
    }

    // 3.4.5 文件比较测试
    #[test]
    fn test_files_equal_identical() {
        // 测试相同文件
    }
    
    #[test]
    fn test_files_equal_different() {
        // 测试不同文件
    }
    
    #[test]
    fn test_file_hash() {
        // 测试文件哈希
    }
    
    #[test]
    fn test_simple_diff_additions() {
        // 测试新增行
    }
    
    #[test]
    fn test_simple_diff_deletions() {
        // 测试删除行
    }
    
    #[test]
    fn test_simple_diff_modifications() {
        // 测试修改行
    }
    
    #[test]
    fn test_simple_diff_empty() {
        // 测试空 diff
    }
}
```

---

### 3.5 搜索功能测试 (`search.rs`)

##### 文件: `tests/search_test.rs`

```rust
#[cfg(test)]
mod search_tests {
    // 3.5.1 文件搜索测试
    #[test]
    fn test_search_files_basic() {
        // 测试基本文件搜索
    }
    
    #[test]
    fn test_search_files_fuzzy() {
        // 测试模糊搜索
    }
    
    #[test]
    fn test_search_files_case_sensitive() {
        // 测试大小写敏感
    }
    
    #[test]
    fn test_search_files_case_insensitive() {
        // 测试大小写不敏感
    }
    
    #[test]
    fn test_search_files_with_wildcard() {
        // 测试通配符
    }
    
    #[test]
    fn test_search_files_no_results() {
        // 测试无结果
    }

    // 3.5.2 文本搜索测试
    #[test]
    fn test_search_text_basic() {
        // 测试基本文本搜索
    }
    
    #[test]
    fn test_search_text_regex() {
        // 测试正则表达式
    }
    
    #[test]
    fn test_search_text_multiline() {
        // 测试多行正则
    }
    
    #[test]
    fn test_search_text_whole_word() {
        // 测试全词匹配
    }
    
    #[test]
    fn test_search_text_case_sensitive() {
        // 测试大小写敏感
    }
    
    #[test]
    fn test_search_text_with_context() {
        // 测试带上下文的搜索
    }

    // 3.5.3 工作区搜索测试
    #[test]
    fn test_search_workspace_basic() {
        // 测试工作区搜索
    }
    
    #[test]
    fn test_search_workspace_exclude() {
        // 测试排除目录
    }
    
    #[test]
    fn test_search_workspace_include() {
        // 测试包含特定类型
    }
    
    #[test]
    fn test_search_workspace_replace_preview() {
        // 测试替换预览
    }
    
    #[test]
    fn test_search_workspace_replace_apply() {
        // 测试应用替换
    }
    
    #[test]
    fn test_search_workspace_large_result() {
        // 测试大量结果
    }

    // 3.5.4 搜索选项测试
    #[test]
    fn test_search_options_max_results() {
        // 测试最大结果数
    }
    
    #[test]
    fn test_search_options_timeout() {
        // 测试搜索超时
    }
    
    #[test]
    fn test_search_options_binary_files() {
        // 测试二进制文件处理
    }
}
```

---

### 3.6 LSP 通信测试 (`lsp.rs`)

##### 文件: `tests/lsp_test.rs`

```rust
#[cfg(test)]
mod lsp_tests {
    // 3.6.1 服务器生命周期测试
    #[test]
    fn test_lsp_start_server() {
        // 测试启动服务器
    }
    
    #[test]
    fn test_lsp_stop_server() {
        // 测试停止服务器
    }
    
    #[test]
    fn test_lsp_list_servers() {
        // 测试服务器列表
    }
    
    #[test]
    fn test_lsp_get_supported_languages() {
        // 测试支持的语言
    }

    // 3.6.2 LSP 请求测试
    #[test]
    fn test_lsp_initialization() {
        // 测试初始化握手
    }
    
    #[test]
    fn test_lsp_capabilities() {
        // 测试能力协商
    }
    
    #[test]
    fn test_lsp_completion() {
        // 测试自动完成
    }
    
    #[test]
    fn test_lsp_hover() {
        // 测试悬停信息
    }
    
    #[test]
    fn test_lsp_goto_definition() {
        // 测试跳转定义
    }
    
    #[test]
    fn test_lsp_find_references() {
        // 测试查找引用
    }
    
    #[test]
    fn test_lsp_signature_help() {
        // 测试签名帮助
    }
    
    #[test]
    fn test_lsp_diagnostics() {
        // 测试诊断信息
    }

    // 3.6.3 格式化测试
    #[test]
    fn test_lsp_document_format() {
        // 测试文档格式化
    }
    
    #[test]
    fn test_lsp_range_format() {
        // 测试范围格式化
    }
    
    #[test]
    fn test_lsp_format_on_type() {
        // 测试键入时格式化
    }

    // 3.6.4 代码操作测试
    #[test]
    fn test_lsp_code_action() {
        // 测试代码动作
    }
    
    #[test]
    fn test_lsp_rename() {
        // 测试重命名
    }
    
    #[test]
    fn test_lsp_rename_conflict() {
        // 测试重命名冲突
    }

    // 3.6.5 错误处理测试
    #[test]
    fn test_lsp_server_crash() {
        // 测试服务器崩溃
    }
    
    #[test]
    fn test_lsp_timeout() {
        // 测试请求超时
    }
    
    #[test]
    fn test_lsp_invalid_request() {
        // 测试无效请求
    }
}
```

---

### 3.7 调试功能测试 (`debug.rs`)

##### 文件: `tests/debug_test.rs`

```rust
#[cfg(test)]
mod debug_tests {
    // 3.7.1 调试适配器测试
    #[test]
    fn test_debug_spawn_adapter() {
        // 测试启动适配器
    }
    
    #[test]
    fn test_debug_kill_adapter() {
        // 测试终止适配器
    }
    
    #[test]
    fn test_debug_send() {
        // 测试发送 DAP 消息
    }
    
    #[test]
    fn test_debug_get_launch_configs() {
        // 测试获取启动配置
    }

    // 3.7.2 调试会话测试
    #[test]
    fn test_debug_start_session() {
        // 测试启动会话
    }
    
    #[test]
    fn test_debug_stop_session() {
        // 测试停止会话
    }
    
    #[test]
    fn test_debug_pause() {
        // 测试暂停
    }
    
    #[test]
    fn test_debug_continue() {
        // 测试继续
    }
    
    #[test]
    fn test_debug_step_over() {
        // 测试逐过程
    }
    
    #[test]
    fn test_debug_step_into() {
        // 测试逐语句
    }
    
    #[test]
    fn test_debug_step_out() {
        // 测试跳出
    }

    // 3.7.3 断点测试
    #[test]
    fn test_debug_set_breakpoint() {
        // 测试设置断点
    }
    
    #[test]
    fn test_debug_remove_breakpoint() {
        // 测试移除断点
    }
    
    #[test]
    fn test_debug_condition_breakpoint() {
        // 测试条件断点
    }
    
    #[test]
    fn test_debug_hit_count_breakpoint() {
        // 测试命中计数断点
    }
    
    #[test]
    fn test_debug_function_breakpoint() {
        // 测试函数断点
    }

    // 3.7.4 变量检查测试
    #[test]
    fn test_debug_stack_trace() {
        // 测试调用堆栈
    }
    
    #[test]
    fn test_debug_scopes() {
        // 测试作用域
    }
    
    #[test]
    fn test_debug_variables() {
        // 测试变量
    }
    
    #[test]
    fn test_debug_evaluate() {
        // 测试表达式求值
    }

    // 3.7.5 DAP 协议测试
    #[test]
    fn test_dap_protocol_handshake() {
        // 测试协议握手
    }
    
    #[test]
    fn test_dap_event_sequence() {
        // 测试事件序列
    }
    
    #[test]
    fn test_dap_response_timeout() {
        // 测试响应超时
    }
}
```

---

### 3.8 扩展管理测试 (`extensions.rs`)

##### 文件: `tests/extensions_test.rs`

```rust
#[cfg(test)]
mod extensions_tests {
    // 3.8.1 安装测试
    #[test]
    fn test_install_extension_vsix() {
        // 测试从 VSIX 安装
    }
    
    #[test]
    fn test_install_extension_url() {
        // 测试从 URL 安装
    }
    
    #[test]
    fn test_install_extension_marketplace() {
        // 测试从市场安装
    }
    
    #[test]
    fn test_install_invalid_vsix() {
        // 测试无效 VSIX
    }
    
    #[test]
    fn test_install_duplicate() {
        // 测试重复安装
    }

    // 3.8.2 卸载测试
    #[test]
    fn test_uninstall_extension() {
        // 测试卸载扩展
    }
    
    #[test]
    fn test_uninstall_nonexistent() {
        // 测试卸载不存在的扩展
    }

    // 3.8.3 列表测试
    #[test]
    fn test_list_installed_extensions() {
        // 测试列出已安装
    }
    
    #[test]
    fn test_search_marketplace() {
        // 测试搜索市场
    }
    
    #[test]
    fn test_search_marketplace_with_query() {
        // 测试带查询的搜索
    }

    // 3.8.4 贡献点测试
    #[test]
    fn test_get_contributions() {
        // 测试获取贡献点
    }
    
    #[test]
    fn test_get_contributions_commands() {
        // 测试获取命令
    }
    
    #[test]
    fn test_get_contributions_menus() {
        // 测试获取菜单
    }
}
```

---

### 3.9 设置管理测试 (`settings.rs`)

##### 文件: `tests/settings_test.rs`

```rust
#[cfg(test)]
mod settings_tests {
    // 3.9.1 读取设置测试
    #[test]
    fn test_settings_get_basic() {
        // 测试基本读取
    }
    
    #[test]
    fn test_settings_get_nested() {
        // 测试嵌套设置
    }
    
    #[test]
    fn test_settings_get_default() {
        // 测试默认值
    }
    
    #[test]
    fn test_settings_get_nonexistent() {
        // 测试读取不存在的设置
    }

    // 3.9.2 更新设置测试
    #[test]
    fn test_settings_update_basic() {
        // 测试基本更新
    }
    
    #[test]
    fn test_settings_update_nested() {
        // 测试嵌套更新
    }
    
    #[test]
    fn test_settings_update_invalid_key() {
        // 测试无效键
    }

    // 3.9.3 JSONC 解析测试
    #[test]
    fn test_parse_jsonc_with_comments() {
        // 测试解析带注释的 JSON
    }
    
    #[test]
    fn test_parse_jsonc_line_comments() {
        // 测试行注释
    }
    
    #[test]
    fn test_parse_jsonc_block_comments() {
        // 测试块注释
    }
    
    #[test]
    fn test_parse_jsonc_invalid() {
        // 测试无效 JSONC
    }
    
    #[test]
    fn test_modify_jsonc_preserve_comments() {
        // 测试修改时保留注释
    }

    // 3.9.4 设置文件测试
    #[test]
    fn test_load_settings() {
        // 测试加载设置
    }
    
    #[test]
    fn test_load_settings_invalid() {
        // 测试加载无效设置
    }
}
```

---

### 3.10 加密功能测试 (`crypto.rs`)

##### 文件: `tests/crypto_test.rs`

```rust
#[cfg(test)]
mod crypto_tests {
    // 3.10.1 哈希测试
    #[test]
    fn test_hash_md5() {
        // 测试 MD5 哈希
    }
    
    #[test]
    fn test_hash_sha256() {
        // 测试 SHA256 哈希
    }
    
    #[test]
    fn test_hash_empty_input() {
        // 测试空输入
    }
    
    #[test]
    fn test_hash_large_input() {
        // 测试大输入
    }
    
    #[test]
    fn test_hash_known_values() {
        // 测试已知值
    }

    // 3.10.2 Base64 测试
    #[test]
    fn test_base64_encode() {
        // 测试编码
    }
    
    #[test]
    fn test_base64_decode() {
        // 测试解码
    }
    
    #[test]
    fn test_base64_roundtrip() {
        // 测试往返编码
    }
    
    #[test]
    fn test_base64_url_safe() {
        // 测试 URL 安全编码
    }
    
    #[test]
    fn test_base64_invalid_input() {
        // 测试无效输入
    }

    // 3.10.3 UUID 测试
    #[test]
    fn test_uuid_generate_v4() {
        // 测试生成 UUID v4
    }
    
    #[test]
    fn test_uuid_parse() {
        // 测试解析 UUID
    }
    
    #[test]
    fn test_uuid_parse_invalid() {
        // 测试解析无效 UUID
    }
}
```

---

### 3.11 编码处理测试

##### 文件: `tests/encoding_test.rs`

```rust
#[cfg(test)]
mod encoding_tests {
    // 3.11.1 UTF-8 测试
    #[test]
    fn test_utf8_basic() {
        // 测试基本 UTF-8
    }
    
    #[test]
    fn test_utf8_chinese() {
        // 测试中文
    }
    
    #[test]
    fn test_utf8_emoji() {
        // 测试 Emoji
    }
    
    #[test]
    fn test_utf8_mixed() {
        // 测试混合内容
    }
    
    #[test]
    fn test_utf8_invalid_sequence() {
        // 测试无效序列
    }

    // 3.11.2 其他编码测试
    #[test]
    fn test_encoding_latin1() {
        // 测试 Latin-1
    }
    
    #[test]
    fn test_encoding_gbk() {
        // 测试 GBK
    }
    
    #[test]
    fn test_encoding_shift_jis() {
        // 测试 Shift-JIS
    }
    
    #[test]
    fn test_encoding_detection() {
        // 测试编码检测
    }

    // 3.11.3 BOM 处理测试
    #[test]
    fn test_bom_utf8() {
        // 测试 UTF-8 BOM
    }
    
    #[test]
    fn test_bom_utf16() {
        // 测试 UTF-16 BOM
    }
    
    #[test]
    fn test_bom_removal() {
        // 测试移除 BOM
    }
}
```

---

### 3.12 集成测试

##### 文件: `tests/integration_test.rs`

```rust
#[cfg(test)]
mod integration_tests {
    // 3.12.1 Git + 编辑器集成测试
    #[test]
    fn test_edit_and_commit_workflow() {
        // 测试编辑并提交工作流
        // 1. 读取文件
        // 2. 修改内容
        // 3. 写入文件
        // 4. git add
        // 5. git commit
    }
    
    #[test]
    fn test_git_blame_display() {
        // 测试 Git Blame 显示
    }

    // 3.12.2 LSP + 编辑器集成测试
    #[test]
    fn test_lsp_completion_in_editor() {
        // 测试编辑器中的 LSP 补全
    }
    
    #[test]
    fn test_lsp_diagnostics_display() {
        // 测试诊断信息显示
    }

    // 3.12.3 调试 + 编辑器集成测试
    #[test]
    fn test_debug_breakpoint_in_editor() {
        // 测试编辑器中的断点
    }
    
    #[test]
    fn test_debug_variable_display() {
        // 测试调试变量显示
    }

    // 3.12.4 终端 + 编辑器集成测试
    #[test]
    fn test_terminal_command_edit_file() {
        // 测试终端命令编辑文件
    }
}
```

---

## 四、TypeScript 前端测试

### 4.1 组件测试

##### 文件: `src/vs/workbench/contrib/scm/**/*.test.ts`

```typescript
describe('SCM Components', function () {
    describe('QuickDiffDecorator', function () {
        it('should show decoration for changed files')
        it('should update decoration on file change')
        it('should handle missing original resource')
    });
    
    describe('SCMHistoryView', function () {
        it('should display commit history')
        it('should handle empty repository')
        it('should show commit details on click')
        it('should navigate to parent commit')
    });
    
    describe('SCMRepositoriesView', function () {
        it('should list all repositories')
        it('should handle single repository')
        it('should detect repository root')
    });
});
```

### 4.2 服务测试

##### 文件: `src/vs/workbench/services/**/*.test.ts`

```typescript
describe('Workbench Services', function () {
    describe('FileService', function () {
        it('should resolve filesystem provider')
        it('should handle read-only resources')
        it('should handle virtual resources')
    });
    
    describe('TextModelService', function () {
        it('should create model from URI')
        it('should register content provider')
        it('should handle model disposal')
    });
});
```

---

## 五、测试执行计划

### 5.1 阶段一：核心功能测试（第 1-2 周）

**目标**：覆盖最高优先级的功能

| 周次 | 测试模块 | 测试用例数 | 目标覆盖 |
|------|---------|-----------|---------|
| 1 | Git 操作 | 50+ | 95% |
| 1 | 文件系统 | 40+ | 95% |
| 2 | 文本处理 | 30+ | 95% |

### 5.2 阶段二：重要功能测试（第 3-4 周）

**目标**：覆盖中优先级功能

| 周次 | 测试模块 | 测试用例数 | 目标覆盖 |
|------|---------|-----------|---------|
| 3 | 终端功能 | 45+ | 90% |
| 3 | 搜索功能 | 30+ | 90% |
| 4 | LSP 通信 | 40+ | 85% |
| 4 | 调试功能 | 35+ | 85% |

### 5.3 阶段三：辅助功能测试（第 5-6 周）

**目标**：覆盖低优先级功能

| 周次 | 测试模块 | 测试用例数 | 目标覆盖 |
|------|---------|-----------|---------|
| 5 | 扩展管理 | 25+ | 90% |
| 5 | 设置管理 | 20+ | 90% |
| 6 | 加密功能 | 15+ | 95% |
| 6 | 编码处理 | 20+ | 90% |

### 5.4 阶段四：集成测试（第 7-8 周）

**目标**：验证模块间协作

| 周次 | 测试类型 | 测试用例数 | 目标覆盖 |
|------|---------|-----------|---------|
| 7 | 组件测试 | 50+ | 80% |
| 7 | 服务测试 | 30+ | 85% |
| 8 | E2E 测试 | 20+ | 70% |

---

## 六、测试覆盖率目标

| 类别 | 当前覆盖率 | 目标覆盖率 |
|------|-----------|-----------|
| Rust 命令模块 | 5% | 85% |
| Rust Crates | 0% | 80% |
| TypeScript 组件 | 0% | 75% |
| TypeScript 服务 | 0% | 80% |

---

## 七、测试基础设施

### 7.1 测试工具

- **Rust**: 内置测试框架 + `cargo test`
- **TypeScript**: Mocha + Chai + ts-node
- **Mock**: `mockall` (Rust), `sinon` (TypeScript)

### 7.2 CI/CD 集成

```yaml
# .github/workflows/test.yml
name: Tests
on: [push, pull_request]

jobs:
  rust-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Run tests
        run: cargo test --all-features
  
  ts-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install dependencies
        run: npm install
      - name: Run tests
        run: npm test
```

### 7.3 测试数据管理

```
tests/
├── fixtures/           # 测试数据
│   ├── repos/         # Git 测试仓库
│   ├── files/         # 测试文件
│   └── configs/       # 测试配置
├── mocks/             # Mock 数据
└── scripts/           # 测试脚本
```

---

## 八、测试最佳实践

### 8.1 测试命名规范

```rust
#[test]
fn test_module_function_behavior() {
    // Arrange - 准备测试数据
    // Act - 执行被测函数
    // Assert - 验证结果
}
```

### 8.2 测试隔离原则

- 每个测试独立运行
- 不依赖测试执行顺序
- 使用 fixture 管理共享数据
- 测试后清理资源

### 8.3 错误处理测试

- 测试正常路径
- 测试边界条件
- 测试错误情况
- 测试安全边界

---

## 九、总结

本测试计划覆盖了 SideX 项目的所有核心功能模块，总计约 **600+** 测试用例。

### 9.1 工作量估算

| 阶段 | 预估时间 | 测试用例数 |
|------|---------|-----------|
| 阶段一 | 2 周 | 120+ |
| 阶段二 | 2 周 | 150+ |
| 阶段三 | 2 周 | 80+ |
| 阶段四 | 2 周 | 100+ |
| **总计** | **8 周** | **450+** |

### 9.2 关键里程碑

- **第 2 周**: Git 和文件系统测试完成
- **第 4 周**: 终端和 LSP 测试完成
- **第 6 周**: 扩展和设置测试完成
- **第 8 周**: 集成测试完成，覆盖率达标

### 9.3 下一步行动

1. ✅ 已完成：Git Graph 相关测试
2. ⬜ 下一项：补充 `fs.rs` 文件系统测试
3. ⬜ 下一项：补充 `text.rs` 文本处理测试
4. ⬜ 下一项：补充 `terminal.rs` 终端测试

---

**文档版本**: 1.0  
**创建日期**: 2026-05-15  
**维护者**: SideX 开发团队  
**审核状态**: 待审核
