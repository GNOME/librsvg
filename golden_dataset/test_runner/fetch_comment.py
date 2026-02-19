#!/usr/bin/env python3
"""
Fetch Comment - Retrieves the agent's PR comment from GitHub.

This module handles:
1. Fetching all comments on a PR
2. Identifying the agent's comment
3. Returning the comment body
"""

import subprocess
import logging
import json
import time

logging.basicConfig(level=logging.INFO, format='%(levelname)s: %(message)s')
logger = logging.getLogger(__name__)


def run_command(cmd):
    """Run a shell command and return the result."""
    logger.debug(f"Running: {cmd}")
    result = subprocess.run(
        cmd,
        shell=True,
        capture_output=True,
        text=True
    )
    return result


def fetch_pr_comments(repo_url, pr_number):
    """
    Fetch all comments on a PR via GitHub CLI.
    
    Args:
        repo_url: GitHub repository (owner/repo)
        pr_number: PR number
    
    Returns:
        List of comment dicts with 'body', 'author', 'createdAt' fields
    """
    result = run_command(
        f'gh pr view {pr_number} --repo "{repo_url}" '
        f'--comments --json body,author,createdAt,id'
    )
    
    if result.returncode != 0:
        logger.error(f"Failed to fetch comments: {result.stderr}")
        return []
    
    try:
        data = json.loads(result.stdout)
        comments = data.get('comments', [])
        return comments
    except json.JSONDecodeError as e:
        logger.error(f"Failed to parse comments: {e}")
        return []


def is_agent_comment(comment):
    """
    Determine if a comment is from the agent.
    
    The agent can be identified by:
    - Author username (if bot account)
    - Comment body patterns
    
    Args:
        comment: Dict with 'body' and 'author' fields
    
    Returns:
        True if this is the agent's comment
    """
    # Check author - common bot patterns
    author = comment.get('author', {})
    username = author.get('login', '').lower()
    
    # Bot account patterns
    bot_patterns = ['bot', 'assistant', 'review', 'agent', 'librsvg']
    if any(pattern in username for pattern in bot_patterns):
        return True
    
    # Check body for agent signatures
    body = comment.get('body', '').lower()
    
    # Agent signature patterns
    signature_patterns = [
        '## categories',
        '## focus areas',
        '## suggestions',
        '## potential issues',
        'pr review',
        'code review',
        'automated review'
    ]
    
    if any(pattern in body for pattern in signature_patterns):
        return True
    
    return False


def fetch_agent_comment(repo_url, pr_number, max_wait=300, poll_interval=5):
    """
    Fetch the agent's comment from a PR, waiting if necessary.
    
    Args:
        repo_url: GitHub repository (owner/repo)
        pr_number: PR number
        max_wait: Maximum seconds to wait for agent comment
        poll_interval: Seconds between polls
    
    Returns:
        Comment body string, or None if not found
    """
    logger.info(f"Fetching agent comment for PR #{pr_number}")
    
    elapsed = 0
    while elapsed < max_wait:
        comments = fetch_pr_comments(repo_url, pr_number)
        
        # Look for agent comment
        for comment in comments:
            if is_agent_comment(comment):
                logger.info(f"Found agent comment (id: {comment.get('id')})")
                return comment.get('body')
        
        logger.debug(f"No agent comment yet, waiting {poll_interval}s...")
        time.sleep(poll_interval)
        elapsed += poll_interval
    
    logger.warning(f"Timeout waiting for agent comment after {max_wait}s")
    return None


def fetch_latest_agent_comment(repo_url, pr_number):
    """
    Fetch the most recent agent comment from a PR.
    
    Unlike fetch_agent_comment, this returns immediately without waiting.
    
    Args:
        repo_url: GitHub repository (owner/repo)
        pr_number: PR number
    
    Returns:
        Comment body string, or None if not found
    """
    comments = fetch_pr_comments(repo_url, pr_number)
    
    # Find agent comments and return the most recent
    agent_comments = [c for c in comments if is_agent_comment(c)]
    
    if agent_comments:
        # Sort by createdAt and return latest
        agent_comments.sort(key=lambda c: c.get('createdAt', ''), reverse=True)
        return agent_comments[0].get('body')
    
    return None


def fetch_all_comments(repo_url, pr_number):
    """
    Fetch all PR comments (for debugging).
    
    Args:
        repo_url: GitHub repository (owner/repo)
        pr_number: PR number
    
    Returns:
        List of all comment bodies
    """
    comments = fetch_pr_comments(repo_url, pr_number)
    return [c.get('body', '') for c in comments]


if __name__ == "__main__":
    # Example usage
    import sys
    
    if len(sys.argv) < 3:
        print("Usage: python fetch_comment.py <owner/repo> <pr_number>")
        sys.exit(1)
    
    repo_url = sys.argv[1]
    pr_number = sys.argv[2]
    
    # Fetch with waiting
    comment = fetch_agent_comment(repo_url, pr_number, max_wait=60, poll_interval=5)
    
    if comment:
        print("=" * 50)
        print("AGENT COMMENT:")
        print("=" * 50)
        print(comment)
    else:
        print("No agent comment found")
