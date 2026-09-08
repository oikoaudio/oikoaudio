#!/usr/bin/env python3
"""Reject release tags that do not match the selected plugin's version."""

import argparse
import os
from products import products


def main() -> None:
    registry = products()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=registry)
    parser.add_argument("--tag", default=os.environ.get("RELEASE_TAG"))
    args = parser.parse_args()
    if not args.tag:
        parser.error("provide --tag or set RELEASE_TAG")

    try:
        registry[args.product].check_tag(args.tag)
    except ValueError as error:
        parser.error(str(error))
    print(f"Release tag matches package version: {args.tag}")


if __name__ == "__main__":
    main()
