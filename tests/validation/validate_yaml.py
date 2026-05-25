#!/usr/bin/env python3
"""
YAML Validation Script
Validates all YAML files for syntax correctness
"""

import sys
import yaml
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed


def validate_yaml_file(filepath):
    """Validate a single YAML file"""
    try:
        with open(filepath, 'r') as f:
            # Handle multi-document YAML files
            list(yaml.safe_load_all(f))
        return (filepath, True, None)
    except yaml.YAMLError as e:
        return (filepath, False, str(e))
    except Exception as e:
        return (filepath, False, str(e))


def main():
    """Main validation function"""
    base_dir = Path(__file__).parent.parent.parent
    
    # Find all YAML files
    yaml_files = []
    for pattern in ['*.yaml', '*.yml']:
        yaml_files.extend(base_dir.rglob(pattern))
    
    # Exclude common directories
    exclude_dirs = ['node_modules', '.git', 'target', 'build', 'dist']
    yaml_files = [
        f for f in yaml_files 
        if not any(excluded in str(f) for excluded in exclude_dirs)
    ]
    
    print(f"Found {len(yaml_files)} YAML files to validate")
    
    # Validate files in parallel
    passed = 0
    failed = 0
    errors = []
    
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = {executor.submit(validate_yaml_file, f): f for f in yaml_files}
        
        for future in as_completed(futures):
            filepath, success, error = future.result()
            if success:
                passed += 1
                print(f"✓ {filepath.relative_to(base_dir)}")
            else:
                failed += 1
                errors.append((filepath, error))
                print(f"✗ {filepath.relative_to(base_dir)}: {error}")
    
    # Summary
    print(f"\n{'='*50}")
    print(f"YAML Validation Summary")
    print(f"{'='*50}")
    print(f"Total:  {passed + failed}")
    print(f"Passed: {passed}")
    print(f"Failed: {failed}")
    
    if errors:
        print(f"\nErrors:")
        for filepath, error in errors:
            print(f"  {filepath}: {error}")
    
    return 0 if failed == 0 else 1


if __name__ == '__main__':
    sys.exit(main())
