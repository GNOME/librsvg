#!/bin/bash

# Here is all of the variables used, 
#  changed by passing through command line arguments
RECOPY=false
INT=false
SYS="no"
REBUILD=false
DIR=no
TMPDIR=/tmp/librsvg

YES=false

CLEANUP=false
RMDISTROIMG=false
RMSYSTEMIMG=false
RMTMP=false

function usage {
        echo "This tool lets you run Librsvg's test suite under a couple different"
        echo "docker containers for testing, it requires sudo privleges (for the docker commands)"
        echo "Use -d [DIRECTORY] pointing at your librsvg Directory"
        echo "Use -s [SYSTEM] to determine what docker container to use (Fedora, OpenSUSE, Debian)"
        echo "Use -h to return this Help"
        echo "Use -i to have it Interactively pause periodically to check output (the cleanup scripts is always interactive unless -y is passed)"
        echo "Use -p to recoPy the librsvg library folder to the tmp directory, removing everything that is there, useful for cleaning the cargo cache"        
		echo "Use -r to Rebuild the build dependencies docker image forcefully"
		echo "Use -t to specify a Temporary directory (default: /tmp/librsvg)"
		echo "Use -y to answer Yes to any prompts (This currently only includes the cleanup scripts)"
		echo "Use -c to Cleanup ALL related docker images (this will not run the test suite)"
}

# Confirm with user to remove Librsvg system docker image 
function confirm {
	if [[ $YES == false ]] 
	then
		echo
		read -p $'Would you like to remove the librsvg docker image with the build dependencies?\x0a(it will take a while to rebuild if removed)\x0aY/N: ' -n 1 -r
		echo 
		if [[ $REPLY =~ ^[Yy]$ ]]
		then
			RMSYSTEMIMG=true
		else
			RMSYSTEMIMG=false
		fi
	fi
}

# Confirm with user to remove $TMPDIR
function confirm_rm_dir {
	if [[ $YES == false ]] 
	then
		echo
		read -p $'Would you like to remove the librsvg files from the tmp directory?\x0aY/N: ' -n 1 -r
		echo 
		if [[ $REPLY =~ ^[Yy]$ ]]
		then
			RMTMP=true
		else
			RMTMP=false
		fi
	fi
}

# Confirm with user to remove distro docker images
function confirm_rm_distro {
	if [[ $YES == false ]] 
	then
		echo
		read -p $'Would you like to remove the base docker system images ie. opensuse?\x0a(do this if you dont plan to build librsvg with this tool in the future, otherwise keep them, it takes a while to build)\x0aY/N: ' -n 1 -r
		echo 
		if [[ $REPLY =~ ^[Yy]$ ]]
		then
			RMDISTROIMG=true
		else
			RMDISTROIMG=false
		fi
	fi
}

# Removes the designated base system docker image
function clean_base_image {
	echo "removing system image librsvg-base-$SYS"
	sudo docker rmi --force librsvg/librsvg-$SYS-base
}

# Removes distro docker images
function clean_distro_image {
	echo "removing base system images"
	sudo docker rmi --force debian
	sudo docker rmi --force opensuse/tumbleweed
	sudo docker rmi --force fedora
}

# Clean the temporary directory
function clean_tmp_dir {
	if [[ "$TMPDIR" == "/" ]] 
	then
		echo "Tried to delete root, exiting"
		exit 1
	fi

	if [[ ! -d "$TMPDIR" ]] 
	then
		echo "$TMPDIR does not exist, exiting"
		exit 0
	fi
	echo "This requires sudo because the build is done with the docker image, so build files cannot be removed without it"
	echo 
	if [[ $INT == true ]]
	then
		read -p "Pausing, press any key to continue, you may be asked for admin password in the next step" -n1 -s
		echo 
	fi
	sudo rm -rf $TMPDIR
}

# Cleanup the various directories and docker images
function cleanup {
	if [[ $CLEANUP == true ]]
	then
		confirm

		if [ $RMSYSTEMIMG == "true" ]
		then
			SYS="opensuse"
			clean_base_image

			SYS="fedora"
			clean_base_image

			SYS="debian"
			clean_base_image

		fi

		confirm_rm_distro
		if [ $RMDISTROIMG == "true" ]
		then
			clean_distro_image
		fi
		
		confirm_rm_dir
		if [ $RMTMP == "true" ]
		then
			clean_tmp_dir
		fi



		exit 0
	fi
	
}

# Check for a docker installation, the script will not continue if Docker is not present.
function check_docker {
	if command -v docker
	then
		echo "Docker found, proceeding"
	else
		echo "No Docker, please install Docker, exiting"
		exit 1
	fi
}

# Check the library directory, defaulting to the current working directory if none is provided
function check_dir {
	echo "Checking if $LIBDIR exists"
	if [[ ! -d "$LIBDIR" ]]
	then
		echo "Library directory: '$LIBDIR' does not exist or isn't set, defaulting to current working directory"
		echo
		LIBDIR=$PWD
	fi
	
	if [[ $LIBDIR == */ ]]
	then
		echo "Directory is good!"
	else
		echo "Directory missing last /, adding"
		echo
		LIBDIR+="/"
	fi
	
	DIR=$PWD
}

# Manually correct for a few different spellings of supported distros 
function check_system {
	echo "Checking what system $SYSTEM is"
	if [[ $SYSTEM == "fedora" ]]
	then
		echo "Fedora"
		SYS="fedora"
	elif [[ $SYSTEM == "Fedora" ]]
	then
		echo "Fedora"
		SYS="fedora"
	elif [[ $SYSTEM == "opensuse" ]]
	then
		echo "OpenSUSE"
		SYS="opensuse"
	elif [[ $SYSTEM == "OpenSUSE" ]]
	then
		echo "OpenSUSE"
		SYS="opensuse"
	elif [[ $SYSTEM == "Debian" ]]
	then
		echo "Debian"
		SYS="debian"
	elif [[ $SYSTEM == "debian" ]]
	then
		echo "Debian"
		SYS="debian"
	else 
		echo "Wrong system selected, must be fedora, opensuse, or debian"
		echo $flag
		echo
		exit 2
	fi
}

# Build the base image, this contains the dependencies for librsvg to be built, and is used to build the system image
function build_base_image {
	if [[ $REBUILD == true ]]
		then
		if [[ $INT == true ]]
			then
				read -p "Rebuilding the Librsvg build dependencies docker container, this will take a moment" -n1 -s
				echo 
		fi

		clean_base_image
		sudo docker build -t librsvg/librsvg-$SYS-base -f tools/docker/librsvg-base/$SYS/Dockerfile tools/docker/librsvg-base/$SYS/.	

	fi

	if [[ $INT == true ]]
	then
		read -p "Building the Librsvg build dependencies docker container, this will take a moment, press any key to continue" -n1 -s
		echo 
	fi

	sudo docker build -t librsvg/librsvg-$SYS-base -f tools/docker/librsvg-base/$SYS/Dockerfile tools/docker/librsvg-base/$SYS/.	
}

#Package librsvg for inclusion in the Docker image
function prepare_librsvg {
	echo "Preparing Librsvg"

	if [[ $RECOPY == false ]] 
		then
			if [[ $INT == true ]]
			then
				read -p "Making a copy, then packaging Librsvg, press any key to continue" -n1 -s
				echo 
			fi

			mkdir $TMPDIR
			echo "Copying librsvg to $TMPDIR"
			echo
			rsync -av --exclude '.git' --exclude 'target' $LIBDIR/ $TMPDIR/

			#Uncomment this line if your distro doesn't have rsync, it'll make a lot of text when copying the git folder, but works
			#cp -r $LIBDIR/. $TMPDIR 
			cd $TMPDIR

			if [[ $INT == true ]]
			then
				read -p "Running autogen to prepare for building in $TMPDIR, press any key to continue" -n1 -s
				echo 
			fi

			#Run autogen, this prepares librsvg for building
			./autogen.sh

	else
		echo "Recopying Librsvg"
		echo
		if [[ ! -d "$TMPDIR" ]] 
		then
			echo "$TMPDIR does not exist, creating"
			mkdir $TMPDIR
		else
			echo "Erasing $TMPDIR and recreating"
			clean_tmp_dir
			mkdir $TMPDIR
		fi

		echo "Copying librsvg to $TMPDIR"
		rsync -av --exclude '.git' --exclude 'target' $LIBDIR/ $TMPDIR/

		#Uncomment this line if your distro doesn't have rsync, it'll make a lot of text when copying the git folder, but works
		#cp -r $LIBDIR/. $TMPDIR
		cd $TMPDIR

		if [[ $INT == true ]]
		then
			read -p "Running autogen to prepare for building in $TMPDIR, then running make clean, press any key to continue" -n1 -s
			echo 
		fi

		#Run autogen, this prepares librsvg for building
		./autogen.sh

	fi	
}

# Runs the built docker image, this runs cargo check automatically attached to the console
function run_docker {
	sudo docker run --name librsvg-$SYS-test -v $TMPDIR:$TMPDIR -w $TMPDIR -t --rm librsvg/librsvg-$SYS-base cargo test 
}

# Get the command line arguments
if [[ ${#} -eq 0 ]]; then
   usage
   exit 1
fi

# Switch through the command line arguments
while getopts "d:s:irpt:ch" flag; do
	case "${flag}" in
		d) LIBDIR=${OPTARG};;
		s) SYSTEM=${OPTARG};;
		i) INT=true;;
		r) REBUILD=true; echo "Rebuilding";;
		p) RECOPY=true; echo "Recopying";;
		t) TMPDIR=${OPTARG};;
		c) CLEANUP=true;;
		h) usage; exit 0;;
		y) YES=true;;
		?) usage; echo "Error: $flag"; exit 1;;
	esac
done

# Runs the script
function main {
	check_docker
	cleanup
	check_dir
	check_system
	build_base_image
	prepare_librsvg
	run_docker

	if [[ $INT == true ]]
	then
		read -p "Tests finished, press any key to exit" -n1 -s
		exit 0
	fi

	echo "Tests finished, exiting"
	exit 0
}

main