#!/usr/bin/env python3
"""
Runner - Main orchestration script for the golden dataset test runner.

This script orchestrates the full test pipeline:
1. Load evaluations from JSON
2. For each evaluation:
   a. Create a PR with the changes
   b. Wait for agent to process
   c. Fetch agent's comment
   d. Parse the comment
   e. Validate against expected
3. Generate reports

Usage:
    python runner.py                    # Run all evaluations
    python runner.py --id 1           # Run single evaluation
    python runner.py --id 1,5,10      # Run specific evaluations
    python runner.py --dry-run        # Show what would run without executing
"""

import argparse
import json
import logging
import os
import sys
import time
import yaml
from datetime import datetime
from pathlib import Path

# Add current directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import create_pr
import fetch_comment
import parse_output
import validator

logging.basicConfig(
    level=logging.INFO,
    format='%(levelname)s: %(message)s'
)
logger = logging.getLogger(__name__)


def load_config(config_path='config.yaml'):
    """Load configuration from YAML file."""
    with open(config_path, 'r') as f:
        return yaml.safe_load(f)


def load_evaluations(config):
    """Load evaluations from JSON file specified in config."""
    evaluations_path = config.get('runner', {}).get('evaluations_file', '../pr_evaluations.json')
    with open(evaluations_path, 'r') as f:
        data = json.load(f)
    # Handle both list format and dict with 'evaluations' key
    if isinstance(data, list):
        return data
    return data.get('evaluations', [])


def setup_output_directory():
    """Create reports directory if it doesn't exist."""
    reports_dir = Path(__file__).parent / 'reports'
    reports_dir.mkdir(exist_ok=True)
    return reports_dir


def save_results(results, reports_dir):
    """Save results to JSON file."""
    timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
    results_file = reports_dir / f'results_{timestamp}.json'
    
    with open(results_file, 'w') as f:
        json.dump(results, f, indent=2)
    
    logger.info(f"Results saved to: {results_file}")
    return results_file


def print_progress(current, total, pr_title):
    """Print progress message."""
    logger.info(f"[{current}/{total}] Processing: {pr_title}")


def run_single_evaluation(evaluation, config):
    """Run a single evaluation end-to-end."""
    github_config = config['github']
    runner_config = config['runner']
    
    repo_url = f"{github_config['owner']}/{github_config['repo']}"
    max_wait = runner_config.get('max_wait_seconds', 300)
    poll_interval = runner_config.get('poll_interval', 5)
    
    result = {
        'evaluation_id': evaluation['id'],
        'pr_title': evaluation['pr_title'],
        'status': 'pending',
        'pr_number': None,
        'pr_url': None,
        'agent_comment': None,
        'parsed_output': None,
        'validation': None,
        'error': None
    }
    
    try:
        # Step 1: Create PR
        logger.info(f"Creating PR for evaluation #{evaluation['id']}...")
        pr_number, pr_url = create_pr.create_pr_for_evaluation(evaluation, config)
        
        if not pr_number:
            result['status'] = 'failed'
            result['error'] = 'Failed to create PR'
            return result
        
        result['pr_number'] = pr_number
        result['pr_url'] = pr_url
        logger.info(f"Created PR #{pr_number}: {pr_url}")
        
        # Step 2: Wait for agent to process
        logger.info(f"Waiting for agent to process PR #{pr_number}...")
        logger.info(f"  (max wait: {max_wait}s, polling every {poll_interval}s)")
        
        # Poll for agent comment
        agent_comment = None
        elapsed = 0
        while elapsed < max_wait:
            time.sleep(poll_interval)
            elapsed += poll_interval
            
            comment = fetch_comment.fetch_latest_agent_comment(repo_url, pr_number)
            if comment:
                agent_comment = comment
                logger.info(f"Agent comment received after {elapsed}s")
                break
            
            logger.debug(f"Still waiting... ({elapsed}s / {max_wait}s)")
        
        if not agent_comment:
            result['status'] = 'timeout'
            result['error'] = f'Agent comment not received after {max_wait}s'
            return result
        
        result['agent_comment'] = agent_comment
        
        # Step 3: Parse agent comment
        logger.info("Parsing agent comment...")
        parsed = parse_output.parse_agent_comment(agent_comment)
        result['parsed_output'] = parsed
        logger.info(f"  Parsed: {len(parsed.get('categories', []))} categories, "
                    f"{len(parsed.get('focus_areas', []))} focus areas")
        
        # Step 4: Validate
        logger.info("Validating against expected output...")
        validation = validator.validate_strict(parsed, evaluation)
        result['validation'] = validation
        
        # Print validation result
        if validation['overall_pass']:
            result['status'] = 'passed'
            logger.info("  ✓ Validation PASSED")
        else:
            result['status'] = 'failed'
            logger.info("  ✗ Validation FAILED")
            logger.info(f"    Score: {validation['overall_score']:.1f}%")
        
        return result
        
    except Exception as e:
        result['status'] = 'error'
        result['error'] = str(e)
        logger.error(f"Error processing evaluation: {e}")
        return result


def run_evaluations(evaluations, config, evaluation_ids=None):
    """
    Run multiple evaluations.
    
    Args:
        evaluations: List of all evaluations
        config: Configuration dict
        evaluation_ids: List of specific IDs to run, or None for all
    
    Returns:
        List of result dicts
    """
    # Filter to specific IDs if provided
    if evaluation_ids:
        evaluations = [e for e in evaluations if e['id'] in evaluation_ids]
    
    if not evaluations:
        logger.warning("No evaluations to run")
        return []
    
    results = []
    total = len(evaluations)
    
    for i, evaluation in enumerate(evaluations, 1):
        print_progress(i, total, evaluation['pr_title'])
        
        result = run_single_evaluation(evaluation, config)
        results.append(result)
        
        # Print brief status
        status_symbol = {
            'passed': '✓',
            'failed': '✗',
            'timeout': '⏱',
            'error': '!'
        }.get(result['status'], '?')
        
        logger.info(f"  Result: {status_symbol} {result['status'].upper()}")
        
        # Save intermediate results
        if i % 5 == 0 or i == total:
            reports_dir = setup_output_directory()
            save_results({'results': results, 'timestamp': datetime.now().isoformat()}, reports_dir)
    
    return results


def print_summary(results):
    """Print summary of test run."""
    total = len(results)
    passed = sum(1 for r in results if r['status'] == 'passed')
    failed = sum(1 for r in results if r['status'] == 'failed')
    timeout = sum(1 for r in results if r['status'] == 'timeout')
    errors = sum(1 for r in results if r['status'] == 'error')
    
    total_score = sum(r.get('validation', {}).get('overall_score', 0) for r in results)
    avg_score = total_score / total if total > 0 else 0
    
    print("\n" + "=" * 60)
    print("TEST RUN SUMMARY")
    print("=" * 60)
    print(f"Total Evaluations: {total}")
    print(f"Passed:            {passed}")
    print(f"Failed:            {failed}")
    print(f"Timeout:           {timeout}")
    print(f"Errors:            {errors}")
    print(f"Pass Rate:         {passed/total*100:.1f}%")
    print(f"Average Score:     {avg_score:.1f}%")
    print("=" * 60)
    
    # List failed evaluations
    failed_evals = [r for r in results if r['status'] == 'failed']
    if failed_evals:
        print("\nFailed Evaluations:")
        for r in failed_evals:
            val = r.get('validation', {})
            print(f"  #{r['evaluation_id']}: {r['pr_title']}")
            print(f"    Score: {val.get('overall_score', 0):.1f}%")
            
            # Show what failed
            if 'validation' in val:
                if not val['categories']['passed']:
                    print(f"    Categories failed: {val['categories']['missing']}")
                if not val['focus_areas']['passed']:
                    print(f"    Focus areas missing: {val['focus_areas']['missing']}")
                if not val['suggestions']['passed']:
                    print(f"    Suggestions missing: {val['suggestions']['missing']}")
                if not val['potential_issues']['passed']:
                    print(f"    Issues missing: {val['potential_issues']['missing']}")
    
    print("=" * 60)


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(
        description='Run golden dataset test suite for PR review assistant'
    )
    parser.add_argument(
        '--id',
        type=str,
        help='Specific evaluation IDs to run (comma-separated, e.g., 1,5,10)'
    )
    parser.add_argument(
        '--config',
        type=str,
        default='config.yaml',
        help='Path to config file (default: config.yaml)'
    )
    parser.add_argument(
        '--dry-run',
        action='store_true',
        help='Show what would run without executing'
    )
    parser.add_argument(
        '--verbose', '-v',
        action='store_true',
        help='Enable verbose logging'
    )
    
    args = parser.parse_args()
    
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Change to script directory
    script_dir = os.path.dirname(os.path.abspath(__file__))
    os.chdir(script_dir)
    
    logger.info("Golden Dataset Test Runner")
    logger.info("=" * 40)
    
    # Load configuration
    logger.info(f"Loading config from: {args.config}")
    try:
        config = load_config(args.config)
    except FileNotFoundError:
        logger.error(f"Config file not found: {args.config}")
        sys.exit(1)
    except yaml.YAMLError as e:
        logger.error(f"Invalid YAML in config: {e}")
        sys.exit(1)
    
    # Load evaluations from config
    evaluations_path = config.get('runner', {}).get('evaluations_file', '../pr_evaluations.json')
    logger.info(f"Loading evaluations from: {evaluations_path}")
    try:
        evaluations = load_evaluations(config)
    except FileNotFoundError:
        logger.error(f"Evaluations file not found: {evaluations_path}")
        sys.exit(1)
    except json.JSONDecodeError as e:
        logger.error(f"Invalid JSON in evaluations: {e}")
        sys.exit(1)
    
    logger.info(f"Loaded {len(evaluations)} evaluations")
    
    # Parse evaluation IDs
    evaluation_ids = None
    if args.id:
        try:
            evaluation_ids = [int(x.strip()) for x in args.id.split(',')]
            logger.info(f"Running evaluations: {evaluation_ids}")
        except ValueError:
            logger.error(f"Invalid evaluation IDs: {args.id}")
            sys.exit(1)
    
    if args.dry_run:
        logger.info("DRY RUN - Showing what would be executed:")
        for e in evaluations:
            if evaluation_ids and e['id'] not in evaluation_ids:
                continue
            print(f"  #{e['id']}: {e['pr_title']}")
            print(f"      Categories: {e.get('categories', [])}")
            print(f"      Difficulty: {e.get('difficulty', 'N/A')}")
        sys.exit(0)
    
    # Run evaluations
    results = run_evaluations(evaluations, config, evaluation_ids)
    
    # Save final results
    reports_dir = setup_output_directory()
    save_results({'results': results, 'timestamp': datetime.now().isoformat()}, reports_dir)
    
    # Print summary
    print_summary(results)
    
    # Exit with appropriate code
    passed = sum(1 for r in results if r['status'] == 'passed')
    failed = sum(1 for r in results if r['status'] == 'failed')
    
    if failed > 0:
        sys.exit(1)
    else:
        sys.exit(0)


if __name__ == '__main__':
    main()
