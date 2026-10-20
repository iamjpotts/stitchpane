
# Document organization

* README.md is a one pager or two pager for humans. Severely limit what it contains to just the essentials
* AGENTS.md is for additional notes not designated for elsewhere
* A specification document set will be established but is not established yet. AGENTS.md should not repeat its contents

# Adding Dependencies

* Always check proposed dependencies for CVEs and advisories before adding them to cargo.toml
* After adding them, recheck with `just deny`

# Resolving Cargo Deny Warnings and Errors

* RUSTSEC and CVE errors - revert the Cargo.toml change, stop, report the cargo deny failure you saw, and ask/prompt for how to proceed
* New licenses - add them to license list
* Duplicate crates (multiple versions of same crate) - add either a `skip` or a `skip-tree` entry with a comment indicating which direct dependency requires the older version of the transient dependency
* Other errors/warnings - stop, report the change, and ask/prompt for how to proceed

# Implementation Constraints and Restrictions

* This application is a GUI, not a TUI
* The use of js, npm, pnpm, typescript, electron, and related technologies is strictly prohibited due to supply chain vulnerability
* The user interface must be native
* An all-rust implementation is preferred, though some C would be acceptable if necessary
* Ubuntu is the target
* Windows is not a target
* MacOS is not a target

# Verifying the application

At a minimum, lints and tests must pass - run `just t` to run the Justfile recipe.

Add other required verifications here and how to execute them as the application is built.

