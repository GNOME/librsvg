#!/usr/bin/env python3
"""
Validator - Strict validation of agent output against expected results.

This module implements strict validation where:
- Categories: Must match exactly (100%)
- Focus Areas: Must match exactly (100%)
- Suggestions: All expected suggestions must be found
- Potential Issues: All expected issues must be identified
"""

import logging
import re

logging.basicConfig(level=logging.INFO, format='%(levelname)s: %(message)s')
logger = logging.getLogger(__name__)


def normalize_text(text):
    """Normalize text for comparison."""
    if isinstance(text, str):
        # Lowercase, remove extra whitespace, strip
        text = text.lower().strip()
        text = re.sub(r'\s+', ' ', text)
    return text


def normalize_set(items):
    """Normalize a list of items for comparison."""
    if not isinstance(items, (list, set)):
        return set()
    return set(normalize_text(item) for item in items if item)


def validate_categories(actual, expected):
    """
    Validate categories - must match exactly.
    
    Args:
        actual: List of actual categories from agent
        expected: List of expected categories from evaluation
    
    Returns:
        Dict with validation results
    """
    actual_set = normalize_set(actual)
    expected_set = normalize_set(expected)
    
    missing = expected_set - actual_set
    extra = actual_set - expected_set
    
    passed = actual_set == expected_set
    
    return {
        'passed': passed,
        'expected': list(expected_set),
        'actual': list(actual_set),
        'missing': list(missing),
        'extra': list(extra),
        'match_percentage': len(actual_set & expected_set) / len(expected_set) * 100 if expected_set else 100
    }


def validate_focus_areas(actual, expected):
    """
    Validate focus areas - must match exactly.
    
    Args:
        actual: List of actual focus areas from agent
        expected: List of expected focus areas from evaluation
    
    Returns:
        Dict with validation results
    """
    actual_set = normalize_set(actual)
    expected_set = normalize_set(expected)
    
    missing = expected_set - actual_set
    extra = actual_set - expected_set
    
    passed = actual_set == expected_set
    
    return {
        'passed': passed,
        'expected': list(expected_set),
        'actual': list(actual_set),
        'missing': list(missing),
        'extra': list(extra),
        'match_percentage': len(actual_set & expected_set) / len(expected_set) * 100 if expected_set else 100
    }


def validate_suggestions(actual, expected):
    """
    Validate suggestions - all expected must be found (subset match).
    
    Args:
        actual: List of actual suggestions from agent
        expected: List of expected suggestions from evaluation
    
    Returns:
        Dict with validation results
    """
    actual_set = normalize_set(actual)
    expected_set = normalize_set(expected)
    
    # For strict: all expected must be found
    found = expected_set & actual_set
    missing = expected_set - actual_set
    
    passed = len(missing) == 0
    
    return {
        'passed': passed,
        'expected': list(expected_set),
        'actual': list(actual_set),
        'found': list(found),
        'missing': list(missing),
        'match_percentage': len(found) / len(expected_set) * 100 if expected_set else 100
    }


def validate_potential_issues(actual, expected):
    """
    Validate potential issues - all expected must be found.
    
    Args:
        actual: List of actual issues from agent
        expected: List of expected issues from evaluation
    
    Returns:
        Dict with validation results
    """
    actual_set = normalize_set(actual)
    expected_set = normalize_set(expected)
    
    found = expected_set & actual_set
    missing = expected_set - actual_set
    
    passed = len(missing) == 0
    
    return {
        'passed': passed,
        'expected': list(expected_set),
        'actual': list(actual_set),
        'found': list(found),
        'missing': list(missing),
        'match_percentage': len(found) / len(expected_set) * 100 if expected_set else 100
    }


def validate_strict(actual, expected):
    """
    Perform strict validation of agent output against expected.
    
    Args:
        actual: Parsed dict from agent's comment
        expected: Expected dict from evaluation
    
    Returns:
        Dict with comprehensive validation results
    """
    expected_review = expected.get('expected_review', {})
    
    results = {
        'evaluation_id': expected.get('id'),
        'pr_title': expected.get('pr_title'),
        'difficulty': expected.get('difficulty'),
        'categories': validate_categories(
            actual.get('categories', []),
            expected.get('categories', [])
        ),
        'focus_areas': validate_focus_areas(
            actual.get('focus_areas', []),
            expected_review.get('focus_areas', [])
        ),
        'suggestions': validate_suggestions(
            actual.get('suggestions', []),
            expected_review.get('suggestions', [])
        ),
        'potential_issues': validate_potential_issues(
            actual.get('potential_issues', []),
            expected_review.get('potential_issues', [])
        )
    }
    
    # Overall pass requires ALL strict checks to pass
    results['overall_pass'] = all([
        results['categories']['passed'],
        results['focus_areas']['passed'],
        results['suggestions']['passed'],
        results['potential_issues']['passed']
    ])
    
    # Calculate overall score
    scores = [
        results['categories']['match_percentage'],
        results['focus_areas']['match_percentage'],
        results['suggestions']['match_percentage'],
        results['potential_issues']['match_percentage']
    ]
    results['overall_score'] = sum(scores) / len(scores) if scores else 0
    
    return results


def generate_validation_report(results):
    """
    Generate a human-readable validation report.
    
    Args:
        results: Dict from validate_strict
    
    Returns:
        String with formatted report
    """
    lines = []
    lines.append("=" * 60)
    lines.append(f"VALIDATION REPORT: {results['pr_title']}")
    lines.append("=" * 60)
    lines.append(f"Evaluation ID: {results['evaluation_id']}")
    lines.append(f"Difficulty: {results['difficulty']}")
    lines.append("")
    
    # Categories
    cat = results['categories']
    status = "PASS" if cat['passed'] else "FAIL"
    lines.append(f"Categories: {status}")
    lines.append(f"  Expected: {cat['expected']}")
    lines.append(f"  Actual:   {cat['actual']}")
    if cat['missing']:
        lines.append(f"  Missing:  {cat['missing']}")
    if cat['extra']:
        lines.append(f"  Extra:    {cat['extra']}")
    lines.append(f"  Match:    {cat['match_percentage']:.1f}%")
    lines.append("")
    
    # Focus Areas
    focus = results['focus_areas']
    status = "PASS" if focus['passed'] else "FAIL"
    lines.append(f"Focus Areas: {status}")
    lines.append(f"  Expected: {focus['expected']}")
    lines.append(f"  Actual:   {focus['actual']}")
    if focus['missing']:
        lines.append(f"  Missing:  {focus['missing']}")
    if focus['extra']:
        lines.append(f"  Extra:    {focus['extra']}")
    lines.append(f"  Match:    {focus['match_percentage']:.1f}%")
    lines.append("")
    
    # Suggestions
    sugg = results['suggestions']
    status = "PASS" if sugg['passed'] else "FAIL"
    lines.append(f"Suggestions: {status}")
    lines.append(f"  Expected: {sugg['expected']}")
    lines.append(f"  Found:    {sugg['found']}")
    if sugg['missing']:
        lines.append(f"  Missing:  {sugg['missing']}")
    lines.append(f"  Match:    {sugg['match_percentage']:.1f}%")
    lines.append("")
    
    # Potential Issues
    issues = results['potential_issues']
    status = "PASS" if issues['passed'] else "FAIL"
    lines.append(f"Potential Issues: {status}")
    lines.append(f"  Expected: {issues['expected']}")
    lines.append(f"  Found:    {issues['found']}")
    if issues['missing']:
        lines.append(f"  Missing:  {issues['missing']}")
    lines.append(f"  Match:    {issues['match_percentage']:.1f}%")
    lines.append("")
    
    # Overall
    overall_status = "PASS" if results['overall_pass'] else "FAIL"
    lines.append("=" * 60)
    lines.append(f"OVERALL: {overall_status}")
    lines.append(f"Score: {results['overall_score']:.1f}%")
    lines.append("=" * 60)
    
    return '\n'.join(lines)


def generate_summary_report(all_results):
    """
    Generate a summary report for multiple evaluations.
    
    Args:
        all_results: List of validation result dicts
    
    Returns:
        String with formatted summary
    """
    total = len(all_results)
    passed = sum(1 for r in all_results if r['overall_pass'])
    failed = total - passed
    
    total_score = sum(r['overall_score'] for r in all_results)
    avg_score = total_score / total if total else 0
    
    lines = []
    lines.append("=" * 60)
    lines.append("TEST RUN SUMMARY")
    lines.append("=" * 60)
    lines.append(f"Total Evaluations: {total}")
    lines.append(f"Passed: {passed}")
    lines.append(f"Failed: {failed}")
    lines.append(f"Pass Rate: {passed/total*100:.1f}%")
    lines.append(f"Average Score: {avg_score:.1f}%")
    lines.append("")
    
    # Failed evaluations
    failed_evals = [r for r in all_results if not r['overall_pass']]
    if failed_evals:
        lines.append("Failed Evaluations:")
        for r in failed_evals:
            lines.append(f"  - #{r['evaluation_id']}: {r['pr_title']}")
    
    lines.append("=" * 60)
    
    return '\n'.join(lines)


if __name__ == "__main__":
    # Example usage
    expected = {
        'id': 1,
        'pr_title': 'Test PR',
        'difficulty': 'easy',
        'categories': ['security', 'performance'],
        'expected_review': {
            'focus_areas': ['input validation', 'memory management'],
            'suggestions': ['add bounds checking'],
            'potential_issues': ['silent return']
        }
    }
    
    actual = {
        'categories': ['security', 'performance'],
        'focus_areas': ['input validation', 'memory management'],
        'suggestions': ['add bounds checking', 'consider alternatives'],
        'potential_issues': ['silent return', 'another issue']
    }
    
    results = validate_strict(actual, expected)
    print(generate_validation_report(results))
