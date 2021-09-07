## Librsvg Docker Tests

Run the librsvg test suite inside a docker container. The Librsvg CI runs on opensuse, so this is a simpler way to run the test suite locally on the same packages that are used by the Gitlab integration. 

Docker requires root for nearly all of its commands so the script will ask for root.

### Usage:
```
This tool lets you run Librsvg's test suite under a couple different docker containers for testing, it requires sudo privleges, which it will ask for in the terminal (this is by the docker commands, which require it)

Use -d [DIRECTORY] pointing at your librsvg Directory
Use -s [SYSTEM] to determine what docker container to use (Fedora, OpenSUSE, Debian)
Use -h to return this Help
Use -i to have it Interactively pause periodically to check output (the cleanup scripts is always interactive unless -y is passed)
Use -p to recoPy the librsvg library folder to the tmp directory, removing everything that is there, useful for cleaning the cargo cache
Use -r to Rebuild the build dependencies docker image forcefully
Use -t to specify a Temporary directory (default: /tmp/librsvg)
Use -y to answer Yes to any prompts (This currently only includes the cleanup scripts)
Use -c to Cleanup ALL related docker images (this will not run the test suite)
```

### Example:
```
If the librsvg folder is in your home directory, run this from your home directory:
~/librsvg/tools/docker/docker-test.sh -d ~/librsvg -s opensuse -i 

This will run it pointing at /home/Username/librsvg (-d) with opensuse tumbleweed docker image (-s), and interactive (-i), meaning it pauses and has the user input a keystroke before continuing, useful for debugging or catching typos.

The first run will take some time as Docker downloads and installs the system, updates the packages, and installs the build requirements, but it's set up so that it won't re-download the system image each time, which takes more disk space but saves on bandwidth.

After the tests run, the links to rendered files in your terminal (in the case that there are errors) will point to the /tmp/librsvg directory, so in some terminals these links can be clicked on to view the files, otherwise the build is found in that folder, and can be accessed from the host system.

What I use, from the librsvg directory:

./tools/docker/docker-test.sh -s opensuse

This passes through the current directory (in this case the librsvg folder, as cloned from git) and runs the test suite in OpenSUSE

```


### Cleanup:
```
To do a full cleanup of the docker images:
./docker-test.sh -c
This requires user input. 

This asks if it should also clear out the tmp directory passed to it, or the default one. It checks if "/" is passed to it, so it shouldn't delete your system. If you answer "no" to clearing out the tmp directory, those files will not be deleted. 

Dangerous: pass -y if using this where user input cannot be provided, but it will delete **all** docker images and the contents of /tmp/librsvg (if -t is not passed, otherwise that directory) without warning. It will not touch the actual librsvg library directory. 
```

### Helpful Docker Commands: 
```
docker image prune
docker container prune

This removes any dangling (not attached to an tagged image) docker images and containers. I would recommend running it once in a while, or you may end up with 100gb of docker containers like me, don't be like me. (disclaimer: I have also been testing all of this so there's been a lot of mishaps and learning)

This tool should use ~3gb of disk space for running the tests with the opensuse image alone. 
If all 3 systems are tested, the disk usage goes up to ~9gb

See your disk usage with:
docker system df
```

### I want to add more OS's to test, how do I do that?
To add a new OS clone one of the `tools/docker/librsvg-base` folders, renaming it to the OS you wish to add, eg. If I wanted to test Alpine, I would rename the folder to "alpine" all lower case, one word, to make things easy. 


Then, go to [Docker Hub](https://hub.docker.com/search?q=&type=image&category=os) and find the OS you wish to add along with the version tag for it, for the alpine example that's `alpine:latest` and put that in the `FROM` portion of the Dockerfile.

LABEL can remain how it is. 

Finally `RUN` needs to be filled in with the package manager command to install the dependencies to build Librsvg. See [COMPILING.md](../../COMPILING.md) for build dependencies, the other Dockerfiles can help here. Essentially it's just looking up what the packages are called on your OS of choice. 

Once you have that out of he way, [docker-test.sh](docker-test.sh) will need a couple edits to be able to build. 
First, inside the `check_system` function copy one of the `elif [[ $SYSTEM == "" ]]` and put it before the `else` at the bottom of that function. Add your system name inside there, copying the style of inside of the other `elif` statements. 

Then, in `cleanup` copy one of the `SYS= ... clean_base_image` lines and change the `SYS` to your OS. 

Finally go to `clean_distro_image` and copy one of the `docker rmi --force` lines, filling in your OS at the end. 

You're done! Now running the script with `./docker-test.sh -s youros` will use your OS, setting the docker images, base image, and everything else correctly!

## How does this tool work?

See the docker-test.sh file for the script itself, below is the architecture of the script.
The dockerfiles in the debian/fedora/opensuse folders have the build dependency install commands that are used to build each of the base images. 

```
┌─────────────────────────────────────────────────────┐
|main                                                 │
| This function runs the following functions one at a |
| time:                                               |
│  check_docker: This makes sure docker is installed  |
|  cleanup                                            │
│  check_dir                                          │
│  check_system                                       │
│  build_base_system                                  │
│  prepare_librsvg                                    │
│  run_docker                                         │
| It's found at the bottom of the script, and pauses  |
|  after everything finishes if -i is passed          |
└─────────────────────────────────────────────────────┘
 Cleanup    │    
            ▼
┌───────────────────────┐     ┌──────────────────────────────────────┐
│cleanup                │  ┌─˃│clean_base_image                      │
│ If -c was passed      │  │  │ Delete librsvg-$SYS-base docker image│
│ Confirm with user     │  │  └──────────────────────────────────────┘
│ Set SYS to each system├──┘  ┌────────────────────────────────┐ ˄
│ Confirm with user ────┼────˃|clean_distro_image              │ |
│ Confirm with user ────┼──┐  | Delete all distro docker images│ |
└───────────────────────┘  │  │ Ie. debian, opensuse/tumbleweed│ |
 Check path |              │  └────────────────────────────────┘ |
            ˅              │  ┌─────────────────────────────┐    |
┌───────────────────────┐  └─˃│clean_tmp_dir                │    |
│check_dir              │     │ Check if $TMPDIR is "/"     │    |
│ checks if $LIBDIR is  |     │ Delete tmp directory on host│    |
| passed with -d, if not|     │ default: /tmp/librsvg       │    |
| defaults to current   |     │ specity with -t             │    |
| directory, then it    |     └─────────────────────────────┘    |
| checks for trailing / |                               ˄        |
│ in library directory, │                               |        |
│ if it doesn't exist   │                               |        |
│ then it appends one.  │                               |        |
│ This makes sure the   │                               |        |
│ later commands don't  |                               |        |
| target the wrong      │                               |        |
| directory.            |                               |        |
└───────────────────────┘                               |        |
 Now the system │                                       |        |
                ˅                                       |        |
┌────────────────────────────────────────┐              |        |
│check_system                            │              |        |
│ this parses a few common spellings of  │              |        |
│ OpenSUSE, Fedora, and Debian           │              |        |
│ to set the $SYS variable to the correct│              |        |
│ OS image. It's convenient to not care  │              |        |
│ about the difference between "OpenSUSE"│              |        |
│ and "opensuse", there's probably a     │              |        |
| better way to do this                  |              |        |
└────────────────────────────────────────┘              |        |
 Build time!    │                                       |        |
                ˅                                       |        |
┌────────────────────────────────────────┐              |        |
│build_base_system                       |              \        |
│ Builds the base docker image: if -r ───┼───────────────┼───────┘
│ then image is rebuilt. If -r is not    │              /
│ passed, then Docker reruns the         │              |
│ Dockerfile command which updates the   │              |
│ existing image using dnf/zypper/apt    │              |
└────────────────────────────────────────┘              |
 Preparations   │                                       |
                ˅                                       |
┌──────────────────────────────────────────────────────┐|
│prepare_librsvg                                       ||
| If -p ───────────────────────────────────────────────┼┘
│ Then creates $TMPDIR if it doesn't exist.            |
| Prepares librsvg, copying it to $TMPDIR, using Rsync │
│ to exclude the git and target folders.               │
│ It then runs autogen in $TMPDIR to prepare for       │
│ building                                             │
└──────────────────────────────────────────────────────┘
 Run Docker   |  
              ˅
┌──────────────────────────────────────────────────────┐
│run_docker                                            |
│ Runs the docker container with this command:         |
│   sudo docker run --name librsvg-$SYS-test \         |
|    -v $TMPDIR:$TMPDIR -w $TMPDIR -t --rm \           |
|    librsvg/librsvg-$SYS-base cargo test              |
|                                                      |
| --name runs the docker container with name           │
│  librsvg-$SYS-test ex: librsvg-opensuse-test         │
|                                                      |
│ -v $TMPDIR:$TMPDIR passes through $TMPDIR on the host│
│  to $TMPDIR inside the container ex: /tmp/librsvg    │
|                                                      |
│ -w $TMPDIR sets the working directory to $TMPDIR     │
|  inside the container                                |
|                                                      |
| -t binds the container's console to the current one  |
|                                                      |
| -rm sets the generated image to self-destruct after  |
|  all processes exit                                  |
|                                                      |
| cargo test runs cargo test inside the container      |
└──────────────────────────────────────────────────────┘

```

