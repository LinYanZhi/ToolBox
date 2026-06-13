@echo off
call mm home
call cargo build -p ls --release
call cargo build -p as --release
copy target\release\as.exe as.exe
copy target\release\ls.exe ls.exe


