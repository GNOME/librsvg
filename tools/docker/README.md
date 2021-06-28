### Librsvg Docker Tests

Run the librsvg test suite inside a docker container. Run the script from within this directory. The Librsvg CI runs on opensuse, so this is a simple-ish way to run the test suite locally on the same packages that are used by the Gitlab integration. 

The build-librsvg.sh script is used internally by the docker-test.sh script, do not run it by itself. (It's not harmful but can unexpectedly modify some files in your tmp directory.)

Usage:
```
This tool lets you run Librsvg's test suite under a couple different docker containers for testing, it requires sudo privleges, which it will ask for in the terminal (this is by the docker commands, which require root)

Use -d [DIRECTORY] pointing at your librsvg directory
Use -s [SYSTEM] to determine what docker container to use (Fedora, OpenSUSE, or Debian)
use -h to return this help
use -i to have it pause periodically to check output
use -r to rebuild the docker image forcefully
use -p to repackage the librsvg image (use in conjunction with -r otherwise the cache will stop changes from taking
use -t to specify a temporary directory (default: /tmp/librsvg)
```

Example:
```
If the librsvg folder is in your home directory:
./docker-test.sh -d ~/librsvg -s opensuse -i 

This will run it pointing at /home/Username/librsvg with opensuse tumbleweed docker image, and interactive, meaning it pauses and has the user input a keystroke before continuing, useful for debugging. 
```


Cleanup:
```
To do a full cleanup of the docker images:
./docker-test.sh -c
This requires user input

I would then suggest either manually rm -r the temp directory you're using (if different than default) or restarting your system to clear out /tmp

Also:
docker image prune

removes any dangling (not attached to an tagged image) docker containers
```

