---
include_toc: true
---

# Project Base
This repo contains initial decent defaults for any new Skynet/Compsoc/Any) project.

## Tools
### Kanban
A basic Kanban can be found by going to the ``Projects`` tab.

### CI/CD
A CI/CD runner is provided and can be found in the ``Actions`` tab.  
This runner is Github actions compatible.

## Components
Each file has a particular reason for existing, often learnt through (painful) experiences.

Not every file is required (such as ``.mailmap``), but most are *strongly* recommended.

### ``README.md``
Every project should have a ``README``/``README.txt``/``README.md``.  
This allows ye to concisely state what the repo is about as well as any quickstart guide.  
It is often done using Markdown (``.md``) since that allows for structured text and HTML compatability.

If the documentation gets too long it is advised to create a ``doc`` folder as outlined below.

### ``LICENSE``
The included ``LICENSE`` is the MIT one.  
You can dual (multi) license by including ``LICENSE-MIT`` and ``LICENSE-APACHE`` as what is common in rust projects.

If a project does not have a license then it is source available, rights reserved.

### ``.gitignore``
This controls what git is permitted to commit and control.  
The file helps ensure that you dont commit code artifacts such as compiled binaries.

It is also a solid defence against committing secrets.

### ``.gitattributes``
The ``.gitattributes`` file preforms two useful roles in this repo:  

1. Ensures a consistent line ending across Windows/Mac/Linux systems.
   * Line endings is useful in a multi device environment.
2. Tells git which files to delegate to LFS.
   * Git is good with text based files.
   * Git-LFS is an addon which stores the files separately and commits a reference to them.
   * This helps ensure teh git repo is not bloated by binary (non text) files.

**Git LFS installer: https://git-lfs.com/**


### ``.forgejo/workflows/check_lfs.yaml``
This is a pipeline config which runs whenever a commit is pushed (``push``), a merge request is updated (``pull_request``) or manually requested (``workflow_dispatch``).

Its purpose is to verify that all files which should be in LFS are in LFS.

When more pipelines are added to a repo then it should be integrated into them.

#### Github compatability
The pipeline is compatible with Github, to do so you need to rename ``.forgejo`` to ``.github``.

### Directories
#### ``src``
It is generally recommended that source code for the project goes into this folder.  
This is an industry psudo-standard.

#### ``doc``
In repo documentation should be put in a ``doc``/``docs``/``documentation`` folder.  
Like ``src`` this is an industry psudo-standard.  

If the documentation is in the ``src`` files this is less important.

### Useful but less important
#### ``.mailmap``
Git works based off of email signatures and a single person may have multiple emails associated with git.  
This file maps multiple emails to a single person, or corrects errros in names.

Documentation: <https://git-scm.com/docs/gitmailmap>

#### ``.gitkeep``
Git tracks files, to it a folder is just a path to a file.  
Thus if a folder is empty then it is invisible to git.  
If you wish to commit a folder (as with ``src`` and ``doc`` in this repo) then you need a placeholder file.  
Once there are other files in the repo then the ``.gitkeep`` can be removed.

## Updating
If any of the files in this repo are updated down the line, such as ``.gitignore``/``.gitattributes``/``.forgejo/workflows/check_lfs.yaml`` then it is recommended to backport the changes here.
