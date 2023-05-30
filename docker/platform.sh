#!/bin/bash

# Used in Docker build to set platform dependent variables

case $TARGETARCH in
	"amd64")
		echo "x86_64-unknown-linux-gnu" > /.platform
		echo "clang" > /.compiler
		curl https://github.com/rui314/mold/releases/download/v1.11.0/mold-1.11.0-x86_64-linux.tar.gz -o mold.tar.gz
		tar -xzf mold.tar.gz
		mv mold/* /
		rm -rf mold mold.tar.gz
	;;
	"arm64")
		echo "aarch64-unknown-linux-gnu" > /.platform
		echo "gcc-aarch64-linux-gnu" > /.compiler
	;;
	"arm")
		echo "armv7-unknown-linux-gnueabihf" > /.platform
		echo "gcc-arm-linux-gnueabihf" > /.compiler
	;;
esac

