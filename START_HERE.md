# AI Agent Instructions: Start Here

**For:** AI Coding Agent  
**Project:** rot - Recursive Operations Tool  
**Mission:** Fix issues and implement comprehensive tests

---

## 📋 What Was Done

### 1. Research Completed ✅
- Analyzed 8 issues from E2E testing
- Identified root causes with code references
- Created detailed fix implementations

### 2. Plan Created ✅
- **`AI_AGENT_EXECUTION_PLAN.md`** - Your execution guide
- **`RESEARCH_AND_TEST_PLAN.md`** - Full research details
- **`TEST_CHECKLIST.md`** - Quick reference

### 3. Issues Identified

| # | Issue | Severity | Fix Ready |
|---|-------|----------|-----------|
| 1 | Task tool timeout | 🔴 Critical | ✅ Yes |
| 2 | No auto-approve flag | 🔴 Critical | ✅ Yes |
| 3 | RLM progress missing | 🔴 Critical | ✅ Yes |
| 4 | Poor error messages | 🟠 High | ✅ Yes |
| 5 | Agent validation | 🟠 High | ✅ Yes |
| 6 | Mode documentation | 🟡 Medium | ✅ Yes |
| 7 | MCP mock server | 🟡 Medium | ✅ Yes |
| 8 | Display glitches | 🟢 Low | ✅ Yes |

---

## 🚀 Your Mission

### Phase 1: Critical Fixes (Week 1)

**Start with Task 1:** Add timeout to Task tool
- Open `AI_AGENT_EXECUTION_PLAN.md`
- Go to "Task 1: Add Timeout Handling to Task Tool"
- Follow Step 1.1, 1.2, 1.3 exactly
- Verify after each step

**Then Task 2:** Add --auto-approve flag
- Follow the plan exactly
- Test after each change

**Then Task 3:** Add RLM progress callbacks
- Follow the plan exactly
- Test after each change

**Then Task 4:** Improve error messages
- Follow the plan exactly
- Test after each change

**Then Task 5-7:** Write tests (100+ tests)
- Follow test templates in the plan
- Run tests frequently
- Fix failures immediately

### Phase 2: Integration Tests (Week 2)
- Task 6: 30 integration tests
- Verify tool combinations work

### Phase 3: E2E Tests (Week 3)
- Task 7: 10 E2E tests
- Verify user workflows work

---

## 📖 How to Use the Plan

### 1. Read `AI_AGENT_EXECUTION_PLAN.md`

This is your main guide. It has:
- Exact file paths to modify
- Exact code to write
- Verification commands
- Success criteria

### 2. Follow Tasks Sequentially

Don't skip ahead. Each task builds on previous ones.

### 3. Verify After Each Step

Run the verification commands. If they fail:
1. Read the error message
2. Check the troubleshooting guide
3. Fix the issue before proceeding

### 4. Run Tests Frequently

```bash
# After each code change
cargo test --lib

# After each task
cargo test --all

# Check quality
cargo clippy --all -- -D warnings
```

---

## 🎯 Success Metrics

You're done when:

- [ ] All 8 tasks completed
- [ ] 140+ tests passing
- [ ] Code compiles cleanly
- [ ] Clippy passes
- [ ] Coverage ≥ 85%
- [ ] All verification commands pass

---

## 🔧 Quick Commands

### Start Task 1
```bash
# Open the plan
cat AI_AGENT_EXECUTION_PLAN.md | grep -A 50 "Task 1:"

# Make the changes
# (Edit files as specified in Task 1)

# Verify
cargo build --package rot-tools
cargo test --package rot-tools test_task_timeout
```

### Run All Tests
```bash
cargo test --all -- --nocapture
```

### Check Quality
```bash
cargo clippy --all -- -D warnings
cargo fmt --all -- --check
```

### Generate Coverage
```bash
cargo tarpaulin --all --out Html
```

---

## 📂 Key Files to Know

### Files You'll Modify
```
crates/rot-tools/src/builtin/task.rs       # Task 1
crates/rot-tools/src/error.rs              # Task 1
crates/rot-cli/src/cli.rs                  # Task 2
crates/rot-cli/src/main.rs                 # Task 2
crates/rot-cli/src/commands/exec.rs        # Task 2, 3, 4
crates/rot-core/src/error.rs               # Task 4
```

### Files You'll Create
```
crates/rot-cli/tests/auto_approve_test.rs
crates/rot-cli/tests/rlm_progress_test.rs
crates/rot-cli/tests/error_messages_test.rs
crates/rot-core/tests/unit/agent_tests.rs
crates/rot-tools/tests/unit/tool_tests.rs
tests/integration/tool_integration.rs
tests/e2e/user_workflows.rs
```

---

## ⚠️ Important Rules

### DO
✅ Follow the plan exactly  
✅ Verify after each step  
✅ Run tests frequently  
✅ Fix errors immediately  
✅ Keep commits small  
✅ Write clear commit messages

### DON'T
❌ Skip steps  
❌ Proceed if verification fails  
❌ Make changes not in the plan  
❌ Ignore test failures  
❌ Skip running tests  
❌ Make large commits

---

## 🐛 If Something Goes Wrong

### Compilation Error
1. Check imports (add missing `use` statements)
2. Check dependencies (add to Cargo.toml)
3. Check syntax (look for typos)
4. Read error message carefully

### Test Failure
1. Run single test: `cargo test test_name -- --nocapture`
2. Read error output
3. Check test expectations
4. Fix code or test as needed

### Lost or Confused
1. Re-read the current task
2. Check what verification should pass
3. Review previous steps
4. If needed, rollback: `git checkout .`

---

## 📊 Progress Tracking

Update `TEST_CHECKLIST.md` as you complete tasks:

```markdown
- [x] Fix 1.1: Add timeout handling
- [x] Fix 1.2: Add --auto-approve flag
- [ ] Fix 1.3: Add RLM progress callbacks
- [ ] Fix 1.4: Improve error messages
...
```

---

## 🎓 Learning Resources

### Rust Testing
- [The Rust Book - Testing](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)

### Project Structure
- Read `AGENTS.md` for project conventions
- Check existing tests for patterns
- Look at similar code in the codebase

---

## 🚦 Ready to Start?

### Pre-flight Check
```bash
# Verify you're in the project root
pwd
# Should show: .../rot

# Verify git is clean
git status
# Should show: nothing to commit

# Verify build works
cargo build
# Should show: Finished

# Verify tests currently pass
cargo test --lib --all
# Should show: test result: ok
```

### Start Task 1
```bash
# Open the execution plan
cat AI_AGENT_EXECUTION_PLAN.md

# Navigate to Task 1
# Start with "Step 1.1: Add Timeout Error Type"
```

---

## 📞 Need Help?

1. Check `AI_AGENT_EXECUTION_PLAN.md` troubleshooting section
2. Check `RESEARCH_AND_TEST_PLAN.md` for detailed explanations
3. Look at existing test files for patterns
4. Check `AGENTS.md` for project conventions

---

## ✅ Definition of Done

Each task is complete when:
- [ ] All code changes made
- [ ] Verification commands pass
- [ ] Tests written and passing
- [ ] No clippy warnings
- [ ] Code formatted
- [ ] Committed with clear message

---

**You have everything you need. Begin with Task 1 in `AI_AGENT_EXECUTION_PLAN.md`. Good luck! 🚀**
