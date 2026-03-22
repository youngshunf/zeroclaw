# 上游合并规范

唤星 fork 仓库与上游 zeroclaw-labs/zeroclaw 的同步流程。

## 分支职责

| 分支 | 角色 | 规则 |
|------|------|------|
| `master` | 上游镜像（默认分支） | 始终 = `upstream/master`，禁止直接提交唤星代码 |
| `huanxing` | 唤星开发分支 | 所有唤星功能在此开发，定期从上游合并更新 |
| `huanxing-clean` | 临时合并工作分支 | 用于合并上游代码并解决冲突，合并完成后推送到远程 |

```
upstream/master ──→ master (镜像) ──→ huanxing-clean (合并) ──→ huanxing (开发)
```

## 同步上游流程

### 第 1 步：更新 master

```bash
git fetch upstream
git checkout master
git merge upstream/master   # 应该是 fast-forward，不会有冲突
git push origin master
```

如果 `git merge` 不是 fast-forward，说明有人误提交到了 master，用 `git reset --hard upstream/master` 修正后 force push。

### 第 2 步：在 huanxing-clean 上合并

```bash
git checkout huanxing-clean
git merge master
```

此步可能产生冲突。冲突解决原则见下方。

### 第 3 步：验证编译

```bash
# 无 feature 编译（确保上游代码完整）
cargo build

# 唤星 feature 编译
cargo build --features huanxing

# 代码检查
cargo clippy --all-targets --features huanxing -- -D warnings
```

三项全部通过后提交合并：

```bash
git commit -m "chore: 合并上游 upstream/master 到 huanxing-clean"
git push origin huanxing-clean
```

### 第 4 步：合并到 huanxing

```bash
git checkout huanxing
git merge huanxing-clean
```

如果有额外冲突（huanxing 上有 huanxing-clean 没有的提交），在此解决。

```bash
cargo build --features huanxing
cargo test --features huanxing
git push origin huanxing
```

## 冲突解决原则

### 优先级

1. 上游逻辑优先 — 上游改了某个函数签名/行为，我们适配
2. 唤星代码保持隔离 — 冲突通常出现在 `#[cfg(feature = "huanxing")]` 附近
3. 不要为了解决冲突而破坏上游代码的完整性

### 常见冲突场景

| 场景 | 处理方式 |
|------|---------|
| 上游修改了我们加过 `cfg(feature)` 的文件 | 保留上游改动，重新添加 feature gate |
| 上游重命名/移动了文件 | 跟随上游，更新 `src/huanxing/` 中的引用 |
| 上游修改了 struct 字段 | 保留上游 struct，更新 `HuanxingExtConfig` 适配 |
| 上游修改了 Cargo.toml 依赖版本 | 采用上游版本，检查唤星依赖是否兼容 |
| 上游删除了我们依赖的 API | 在 `src/huanxing/` 中实现替代方案或 wrapper |

### 冲突解决后检查清单

- [ ] `cargo build` 通过（无 feature，确保上游完整）
- [ ] `cargo build --features huanxing` 通过
- [ ] `cargo clippy --all-targets --features huanxing -- -D warnings` 无警告
- [ ] 上游文件只包含 `#[cfg(feature = "huanxing")]` 行，无其他唤星逻辑
- [ ] 没有意外删除上游的新功能或修复

## GitHub Sync Fork

master 作为默认分支且与上游保持一致后，GitHub 页面的 "Sync fork" 按钮可以正常使用。但建议用命令行操作，更可控。

如果 GitHub 提示 "X commits ahead, Y commits behind"：
- master 分支出现此提示 → 说明 master 被污染了，需要 `reset --hard upstream/master`
- huanxing 分支出现此提示 → 正常，因为 huanxing 包含唤星代码

## 同步频率建议

- 日常开发：每 1-2 周同步一次上游
- 上游有重大更新（大版本、安全修复）：立即同步
- 唤星功能开发密集期：可延后，但不超过 1 个月

## 紧急情况处理

### master 被误提交了唤星代码

```bash
git checkout master
git reset --hard upstream/master
git push origin master --force
```

### huanxing-clean 状态混乱

```bash
# 从 huanxing 重新创建
git checkout huanxing
git branch -D huanxing-clean
git checkout -b huanxing-clean
git push origin huanxing-clean --force
```

### 合并后发现严重问题需要回退

```bash
git checkout huanxing
git revert -m 1 <merge-commit-hash>   # 回退合并提交
git push origin huanxing
```
