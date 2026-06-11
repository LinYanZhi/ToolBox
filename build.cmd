@echo off
call mm home
call cargo build -p ls --release
call cargo build -p as --release
