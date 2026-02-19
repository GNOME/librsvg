# Golden Dataset Test Runner

Test runner for validating PR review assistant agent against golden dataset evaluations.

## Overview

This test runner orchestrates the full test pipeline:
1. Creates PRs with test code changes
2. Waits for the agent to process and comment
3. Parses the agent's markdown comment
4. Validates the output against expected results (strict validation)

## Directory Structure

```
golden_dataset/
├── evaluations.json          # 25 test evaluations
└── test_runner/
    ├── config.yaml          # Configuration
    ├── requirements.txt     # Python dependencies
    ├── runner.py           # Main orchestration script
    ├── create_pr.py        # Creates branch, commits, opens PR
    ├── fetch_comment.py    # Fetches agent's PR comment
    ├── parse_output.py    # Parses markdown to structured data
    ├── validator.py       # Strict validation logic
    └── reports/           # Test results output
```

## Prerequisites

1. **GitHub CLI (`gh`)** - Must be authenticated
   ```bash
   gh auth login
   ```

2. **Python 3.9+** with dependencies
   ```bash
   pip install -r requirements.txt
   ```

3. **GitHub Repository** - Forked librsvg with:
   - Webhook configured to trigger your agent
   - Agent should post review comments in markdown format

4. **GitHub Token** - Set `GH_TOKEN` environment variable or use `gh auth`

## Configuration

Edit `config.yaml`:

```yaml
github:
  owner: "your-org"        # GitHub organization/username
  repo: "librsvg"         # Repository name
  base_branch: "main"     # Base branch for PRs

runner:
  repo_path: "/path/to/local/librsvg-fork"  # Local clone of fork
  max_wait_seconds: 300   # Max time to wait for agent
  poll_interval: 5         # Seconds between polls
```

## Usage

### Run All Evaluations

```bash
cd golden_dataset/test_runner
python runner.py
```

### Run Specific Evaluation

```bash
python runner.py --id 1
```

### Run Multiple Evaluations

```bash
python runner.py --id 1,5,10
```

### Dry Run (Show What Would Run)

```bash
python runner.py --dry-run
```

### Verbose Output

```bash
python runner.py -v
```

## Agent Output Format

The test runner expects the agent to post comments in this format:

```markdown
## Categories
- security
- performance

## Focus Areas
- Input validation
- Memory management

## Suggestions
- Add bounds checking to prevent overflow
- Consider using saturating arithmetic

## Potential Issues
- Silent return may hide errors
- Backward compatibility concerns
```

## Validation (Strict)

Strict validation requires:
- **Categories**: Must match exactly
- **Focus Areas**: Must match exactly
- **Suggestions**: All expected suggestions must be found
- **Potential Issues**: All expected issues must be identified

## Output

Results are saved to `reports/`:
- `results_<timestamp>.json` - Detailed results

Console output shows:
- Pass/fail status for each evaluation
- Summary with pass rate and average score

## Example Output

```
INFO: [1/25] Processing: Refactor clamp function to use standard library
INFO:   Result: ✓ PASSED
INFO: [2/25] Processing: Add null check safety improvements to utf8_cstr
INFO:   Result: ✗ FAILED
...
INFO: ============================================================
INFO: TEST RUN SUMMARY
INFO: ============================================================
INFO: Total Evaluations: 25
INFO: Passed:            18
INFO: Failed:            7
INFO: Pass Rate:         72.0%
INFO: Average Score:     85.2%
INFO: ============================================================
```

## Troubleshooting

### "Failed to create PR"

- Ensure `gh` is authenticated: `gh auth status`
- Check repo path exists and is a git repo

### "Agent comment not received"

- Verify webhook is configured and triggering agent
- Check `max_wait_seconds` and `poll_interval` in config
- Check agent logs for errors

### Validation Failures

- Review `reports/results_*.json` for detailed validation data
- Check if agent output format matches expected markdown structure
- Review validator.py for exact matching criteria

## Development

### Adding New Tests

Add evaluations to `evaluations.json`:
```json
{
  "id": 26,
  "pr_title": "New test PR",
  "pr_description": "Description...",
  "changes": [...],
  "expected_review": {...}
}
```

### Modifying Validation

Edit `validator.py` for custom validation logic.
