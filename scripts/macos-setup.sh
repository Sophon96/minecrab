# This is setup for the macOS build process.
# Run this script once per command line session, and thereafter run 'cargo build'
# and 'cargo run' plain to compile and execute the project.

# This is needed for SDL3 linkage errors.

# This assumes you are using homebrew for your SDL3 installation.

if ! brew ls --versions sdl3 &> /dev/null;
then
	echo "SDL3 not installed via brew: doing nothing"
	return 1
fi

if ! brew ls --versions pkg-config &> /dev/null;
then
    echo "pkg-config not installed via brew: pkg-config must be installed via brew to correctly detect SDL3"
    return 1
fi

if ! pkg-config --exists sdl3;
then
    echo "pkg-config was unable to find SDL3: something is wrong with SDL3 or pkg-config"
    return 1
fi

export LIBRARY_PATH="$LIBRARY_PATH:$(brew --prefix)/lib"
