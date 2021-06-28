#!/bin/bash
REPACKAGE=false
INT=false
SYS="no"
REBUILD=false
DIR=no
TMPDIR=/tmp/librsvg
CLEANUP=false

function usage {
        echo "This tool lets you run Librsvg's test suite under a couple different"
        echo "docker containers for testing, it requires sudo privleges (for the docker commands)"
        echo "Use -d [DIRECTORY] pointing at your librsvg Directory"
        echo "Use -s [SYSTEM] to determine what docker container to use (Fedora, OpenSUSE, Debian)"
        echo "Use -h to return this Help"
        echo "Use -i to have it Interactively pause periodically to check output"
        echo "Use -r to Rebuild the docker image forcefully"
        echo "Use -p to rePackage the librsvg tar (use in conjunction with -r otherwise the cache will stop changes from taking"
		echo "Use -t to specify a Temporary directory (default: /tmp/librsvg)"
		echo "Use -c to Cleanup ALL related docker images (this will not run the test suite)"
}

function copy_build_script {
	#echo $PWD
	echo "Copying build-librsvg.sh to $SYS folder"
	cp $PWD/build-librsvg.sh $SYS/
}

#Package librsvg for inclusion in the Docker image
function package_librsvg {
	echo "Packaging Librsvg"

	if [[ ! -f "$SYS/librsvg.tar.gz" ]]
	then
		if [[ $INT == true ]]
		then
			read -p "Making a copy, then running make clean and packaging Librsvg, press any key to continue" -n1 -s
		fi

		mkdir $TMPDIR
		echo "Copying librsvg to $TMPDIR"
		cp -r $LIBDIR/. $TMPDIR
		cd $TMPDIR

		#Run autogen, this prepares librsvg for building, and allows make clean to be ran
		./autogen.sh
		#run make clean which makes the resulting tar much smaller.
		make clean
		cd $DIR
		tar -cvzf $SYS/librsvg.tar.gz -C $TMPDIR . --xform='s!^\./!!'
	fi

	if [[ $REPACKAGE == true ]]
	then
		echo "Repackaging Librsvg"
		mkdir $TMPDIR
		echo "Copying librsvg to $TMPDIR"
		cp -r $LIBDIR/. $TMPDIR
		cd $TMPDIR

		#Run autogen, this prepares librsvg for building, and allows make clean to be ran
		./autogen.sh
		#run make clean which makes the resulting tar much smaller.
		make clean
		cd $DIR
		tar -cvzf $SYS/librsvg.tar.gz -C $TMPDIR . --xform='s!^\./!!'
	fi
}

#build the base image, this contains the dependencies for librsvg to be built, and is used to build the system image
function build_base_image {
	sudo docker build -t librsvg/librsvg-base-$SYS -f librsvg-base/$SYS/Dockerfile librsvg-base/$SYS/.	
}

#build the system image, this is the image which librsvg goes into and is built with
function build_system_image {
	sudo docker build -t librsvg/librsvg-$SYS -f $SYS/Dockerfile $SYS/.
}

#removes the system image and rebuilds it, this doesn't touch the system images
function rebuild_docker {
	echo "removing old image"
	sudo docker rmi librsvg/librsvg-$SYS
	sudo docker build -t librsvg/librsvg-$SYS --no-cache -f $SYS/Dockerfile $SYS/.
}

#Build the docker image, using $SYS and $LIBDIR to determine where and what library
function build_docker {
	if [[ $REBUILD == false ]]
	then
		if [[ $INT == true ]]
		then
			read -p "Building Docker with cache and settings $SYS, $LIBDIR Press any key to continue" -n1 -s
		fi
		echo "Building Docker System $SYS with cache"
		build_system_image
	else
		if [[ $INT == true ]]
		then
			read -p "Rebuilding Docker with settings $SYS, $LIBDIR Press any key to continue" -n1 -s
		fi

		rebuild_docker
	fi
}

#removes the designated system image
function remove_system_image {
	echo "removing system image librsvg-base-$SYS"
	sudo docker rmi librsvg/librsvg-base-$SYS
}

function remove_librsvg_image {
	echo "removing librsvg image librsvg-$SYS"
	sudo docker rmi librsvg/librsvg-$SYS
}

function cleanup {
	if [[ $CLEANUP == true ]]
	then
		confirm
		SYS=opensuse
		remove_librsvg_image
		remove_system_image
		rm $SYS/librsvg.tar.gz
		rm $SYS/build-librsvg.sh
		SYS=fedora
		remove_librsvg_image
		remove_system_image
		rm $SYS/librsvg.tar.gz
		rm $SYS/build-librsvg.sh
		SYS=debian
		emove_librsvg_image
		remove_system_image
		rm $SYS/librsvg.tar.gz
		rm $SYS/build-rsvg.sh
		exit 0
	fi
	
}

function confirm {
	echo "Are you sure? This will remove all Docker images and the packaged librsvg.tar.gz"
	select yn in "Yes" "No"; do
    	case $yn in
    	    Yes ) break;;
    	    No ) exit 1;;
   		esac
	done
}

#runs the built docker image, this runs build_librsvg.sh automatically attached to the console
function run_docker {
	sudo docker run -it librsvg/librsvg-$SYS
}

function check_dir {
	echo "Checking if $LIBDIR exists"
	if [[ ! -d "$LIBDIR" ]]
	then
		echo "$LIBDIR does not exist, exiting"
		exit 2
	fi
	
	if [[ $LIBDIR == */ ]]
	then
		echo "Directory is good!"
	else
		echo "Directory missing last /, adding"
		LIBDIR+="/"
	fi
	
	DIR=$PWD
}

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
		exit 2
	fi
}

if [[ ${#} -eq 0 ]]; then
   usage
   exit 1
fi

while getopts "d:s:irpt:ch" flag; do
	case "${flag}" in
		d) LIBDIR=${OPTARG};;
		s) SYSTEM=${OPTARG};;
		i) INT=true;;
		r) REBUILD=true; echo "Rebuilding";;
		p) REPACKAGE=true; echo "Repackaging";;
		t) TMPDIR=${OPTARG};;
		c) CLEANUP=true;;
		h) usage; exit 0;;
		?) usage; echo "Error: $flag"; exit 1;;
	esac
done

# Runs the script then cleans up (if there's write permissions, which there should be)
function main {
	cleanup
	check_dir
	check_system
	copy_build_script
	package_librsvg
	build_base_image
	build_docker
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