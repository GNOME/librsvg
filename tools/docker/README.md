## Librsvg Docker Tests

Run the librsvg test suite inside a docker container. Run the script from within this directory. The Librsvg CI runs on opensuse, so this is a simple-ish way to run the test suite locally on the same packages that are used by the Gitlab integration. 

The build-librsvg.sh script is used internally by the docker-test.sh script, do not run it by itself. (It's not harmful but can unexpectedly modify some files in your tmp directory.)

Docker requires root for nearly all of its commands so the script will ask for root. 

### Usage:
```
This tool lets you run Librsvg's test suite under a couple different docker containers for testing, it requires sudo privleges, which it will ask for in the terminal (this is by the docker commands, which require root)

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
If the librsvg folder is in your home directory:
~/librsvg/tools/docker/docker-test.sh -d ~/librsvg -s opensuse -i 

This will run it pointing at /home/Username/librsvg (-d) with opensuse tumbleweed docker image (-s), and interactive (-i), meaning it pauses and has the user input a keystroke before continuing, useful for debugging or catching typos. 

The first run will take some time as Docker downloads and installs the system, updates the packages, and installs the build requirements, but it's set up so that it won't re-download the system image each time, which takes more disk space but saves on bandwidth.

What I use, from the librsvg directory:

tools/docker/docker-test.sh -s opensuse
```


### Cleanup:
```
To do a full cleanup of the docker images:
./docker-test.sh -c
This requires user input

This asks if it should also clear out the tmp directory passed to it, or the default one. It checks if "/" is passed to it, so it shouldn't delete your system. If you answer "no" to clearing out the tmp directory, the files will not be deleted. 

Also:
docker image prune
docker container prune

This removes any dangling (not attached to an tagged image) docker images and containers. I would recommend running it once in a while, or you may end up with 100gb of docker containers like me, don't be like me. (disclaimer: I have also been testing all of this so there's been a lot of mishaps and learning)

This tool should use ~3gb of disk space for running the tests with the opensuse image alone. 
If all 3 systems are tested, the disk usage goes up to ~9gb

See your disk usage with:
docker system df
```

### How does this tool work?


